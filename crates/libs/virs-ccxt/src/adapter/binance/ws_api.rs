//! Binance WebSocket API 客户端（现货用户数据流替代方案）。
//!
//! 用途：替代已废弃的 `POST /api/v3/userDataStream` + `wss://stream.binance.com/ws/<listenKey>` 方案。
//! 币安在 2025-04-25 Changelog 中宣布废弃 listenKey REST 端点，推荐使用 WebSocket API 的
//! `userDataStream.subscribe` 方法（要求 Ed25519 API Key）。
//!
//! 连接流程：
//! 1. 连接 `wss://ws-api.binance.com/ws-api/v3`
//! 2. 发送 `session.logon`（用 Ed25519 签名认证）
//! 3. 发送 `userDataStream.subscribe`（订阅用户数据流，无需 listenKey）
//! 4. 接收 executionReport / outboundAccountPosition 等事件
//! 5. 定期发送 `userDataStream.ping` 保活
//!
//! 参考文档：
//! - https://developers.binance.com/docs/binance-spot-api-docs/web-socket-api
//! - https://developers.binance.com/docs/binance-spot-api-docs/web-socket-api/user-data-stream-requests

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use virs_error::ExchangeError;

use super::BinanceEd25519Signer;
use crate::adapter::binance::order_ws::BinanceOrderMessage;
use crate::ws_types::WsFeedEvent;

/// 现货 WebSocket API 端点
const SPOT_WS_API_URL: &str = "wss://ws-api.binance.com/ws-api/v3";

/// Binance WebSocket API 用户数据流客户端（基于 Ed25519 认证）。
///
/// 与 `BinanceOrderWs`（listenKey 模式）的区别：
/// - 无需调用 `POST /api/v3/userDataStream` 获取 listenKey
/// - 通过 `session.logon` 用 Ed25519 签名直接认证
/// - 通过 `userDataStream.subscribe` 直接订阅，不需要把 listenKey 拼到 URL
/// - 保活用 `userDataStream.ping` 替代 `PUT /api/v3/userDataStream`
///
/// 生命周期：`start()` 后客户端结构体可被丢弃，后台 task 通过 `event_tx.send()`
/// 失败（接收方关闭）或 `stop()` 设置 `running=false` 来退出。
pub struct BinanceWsApiOrderWs {
    url: String,
    ed25519_signer: BinanceEd25519Signer,
    reconnect_delay_secs: u64,
    max_reconnect_delay_secs: u64,
    ws_ping_interval_secs: u64,
    /// 用户数据流保活间隔（币安要求每 60 分钟内 ping 一次，默认 30 分钟）
    user_data_ping_interval_secs: u64,
    ws_max_lifetime_secs: u64,
    running: Arc<AtomicBool>,
    request_id: Arc<AtomicU64>,
}

impl BinanceWsApiOrderWs {
    /// 创建现货用户数据流 WebSocket API 客户端
    pub fn new_spot(ed25519_signer: BinanceEd25519Signer) -> Self {
        Self {
            url: SPOT_WS_API_URL.to_string(),
            ed25519_signer,
            reconnect_delay_secs: 1,
            max_reconnect_delay_secs: 60,
            ws_ping_interval_secs: 30,
            user_data_ping_interval_secs: 30 * 60,
            ws_max_lifetime_secs: 23 * 3600,
            running: Arc::new(AtomicBool::new(false)),
            request_id: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// 启动 WS 连接，将订单事件发送到 event_tx。
    ///
    /// 后台 task 在 `event_tx` 接收方关闭或调用 `stop()` 后自动退出。
    /// 本方法返回后 `self` 可被丢弃，不影响后台 task 运行。
    pub fn start(&self, event_tx: mpsc::Sender<WsFeedEvent>) {
        if self.running.load(Ordering::Relaxed) {
            return;
        }
        self.running.store(true, Ordering::Relaxed);

        let url = self.url.clone();
        let ed25519_signer = self.ed25519_signer.clone();
        let running = self.running.clone();
        let request_id = self.request_id.clone();
        let reconnect_delay_secs = self.reconnect_delay_secs;
        let max_reconnect_delay_secs = self.max_reconnect_delay_secs;
        let ws_ping_interval_secs = self.ws_ping_interval_secs;
        let user_data_ping_interval_secs = self.user_data_ping_interval_secs;
        let ws_max_lifetime_secs = self.ws_max_lifetime_secs;

        tokio::spawn(async move {
            let mut reconnect_delay = reconnect_delay_secs;

            while running.load(Ordering::Relaxed) {
                let connect_start = tokio::time::Instant::now();

                tracing::debug!("[BinanceWsApiOrderWs] Connecting to {}...", url);

                match connect_async(&url).await {
                    Ok((ws_stream, _)) => {
                        tracing::debug!("[BinanceWsApiOrderWs] Connected to {}", url);
                        reconnect_delay = reconnect_delay_secs;

                        if event_tx
                            .send(WsFeedEvent::ConnectionChanged { connected: true })
                            .await
                            .is_err()
                        {
                            tracing::warn!(
                                "[BinanceWsApiOrderWs] Event channel closed during connect, stopping"
                            );
                            running.store(false, Ordering::Relaxed);
                            break;
                        }

                        let (mut write, mut read) = ws_stream.split();

                        // 1) session.logon 认证
                        let logon_id = request_id.fetch_add(1, Ordering::Relaxed);
                        match build_session_logon_request(&ed25519_signer, logon_id) {
                            Ok(logon_msg) => {
                                let logon_text = match serde_json::to_string(&logon_msg) {
                                    Ok(s) => s,
                                    Err(e) => {
                                        tracing::error!(
                                            error = %e,
                                            "[BinanceWsApiOrderWs] Failed to serialize logon_msg — skipping send"
                                        );
                                        continue;
                                    }
                                };
                                tracing::debug!(
                                    "[BinanceWsApiOrderWs] Sending session.logon (id={})",
                                    logon_id
                                );
                                if write.send(Message::Text(logon_text.into())).await.is_err() {
                                    tracing::error!(
                                        "[BinanceWsApiOrderWs] Failed to send session.logon"
                                    );
                                    continue;
                                }
                            }
                            Err(e) => {
                                tracing::error!(
                                    error = %e,
                                    "[BinanceWsApiOrderWs] Failed to build session.logon request"
                                );
                                continue;
                            }
                        }

                        // 2) 等待 logon 响应（响应中 status=200 表示认证成功）
                        let logon_ok =
                            wait_for_response(&mut read, logon_id, Duration::from_secs(10)).await;

                        match logon_ok {
                            Ok(true) => {
                                tracing::info!(
                                    "[BinanceWsApiOrderWs] session.logon succeeded, user data stream authenticated"
                                );
                            }
                            Ok(false) => {
                                tracing::error!(
                                    "[BinanceWsApiOrderWs] session.logon returned non-200 status; \
                                     check if API key is Ed25519 type"
                                );
                                let _ = event_tx
                                    .send(WsFeedEvent::ConnectionChanged { connected: false })
                                    .await;
                                tokio::time::sleep(Duration::from_secs(reconnect_delay)).await;
                                reconnect_delay =
                                    (reconnect_delay * 2).min(max_reconnect_delay_secs);
                                continue;
                            }
                            Err(e) => {
                                tracing::error!(
                                    error = %e,
                                    "[BinanceWsApiOrderWs] Failed to receive session.logon response"
                                );
                                continue;
                            }
                        }

                        // 3) userDataStream.subscribe 订阅
                        let sub_id = request_id.fetch_add(1, Ordering::Relaxed);
                        let sub_msg = serde_json::json!({
                            "id": sub_id,
                            "method": "userDataStream.subscribe",
                            "params": {}
                        });
                        let sub_text = match serde_json::to_string(&sub_msg) {
                            Ok(s) => s,
                            Err(e) => {
                                tracing::error!(
                                    error = %e,
                                    "[BinanceWsApiOrderWs] Failed to serialize sub_msg — skipping send"
                                );
                                continue;
                            }
                        };
                        tracing::debug!(
                            "[BinanceWsApiOrderWs] Sending userDataStream.subscribe (id={})",
                            sub_id
                        );
                        if write.send(Message::Text(sub_text.into())).await.is_err() {
                            tracing::error!(
                                "[BinanceWsApiOrderWs] Failed to send userDataStream.subscribe"
                            );
                            continue;
                        }

                        let sub_ok =
                            wait_for_response(&mut read, sub_id, Duration::from_secs(10)).await;
                        match sub_ok {
                            Ok(true) => {
                                tracing::info!(
                                    "[BinanceWsApiOrderWs] userDataStream.subscribe succeeded"
                                );
                            }
                            Ok(false) => {
                                tracing::error!(
                                    "[BinanceWsApiOrderWs] userDataStream.subscribe returned non-200 status"
                                );
                                continue;
                            }
                            Err(e) => {
                                tracing::error!(
                                    error = %e,
                                    "[BinanceWsApiOrderWs] Failed to receive subscribe response"
                                );
                                continue;
                            }
                        }

                        // 4) 进入主循环：接收事件 + 心跳 + 用户数据流保活
                        let ws_ping = Duration::from_secs(ws_ping_interval_secs);
                        let user_ping = Duration::from_secs(user_data_ping_interval_secs);
                        let max_lifetime = Duration::from_secs(ws_max_lifetime_secs);
                        let mut ws_ping_tick = tokio::time::interval(ws_ping);
                        let mut user_ping_tick = tokio::time::interval(user_ping);
                        // 第一次 tick 立即触发，跳过
                        ws_ping_tick.tick().await;
                        user_ping_tick.tick().await;

                        loop {
                            if !running.load(Ordering::Relaxed) {
                                break;
                            }
                            if connect_start.elapsed() > max_lifetime {
                                tracing::debug!(
                                    "[BinanceWsApiOrderWs] Max lifetime reached, reconnecting..."
                                );
                                break;
                            }

                            tokio::select! {
                                msg = read.next() => {
                                    #[allow(clippy::collapsible_match)]
                                    match msg {
                                        Some(Ok(Message::Text(text))) => {
                                            if handle_text_message(&text, &event_tx).await {
                                                running.store(false, Ordering::Relaxed);
                                                break;
                                            }
                                        }
                                        Some(Ok(Message::Binary(b))) => {
                                            if let Ok(text) = String::from_utf8(b.to_vec()) {
                                                if handle_text_message(&text, &event_tx).await {
                                                    running.store(false, Ordering::Relaxed);
                                                    break;
                                                }
                                            }
                                        }
                                        Some(Ok(Message::Ping(p))) => {
                                            let _ = write.send(Message::Pong(p)).await;
                                        }
                                        Some(Ok(Message::Close(_))) => {
                                            tracing::debug!("[BinanceWsApiOrderWs] Server closed connection");
                                            break;
                                        }
                                        Some(Err(e)) => {
                                            tracing::warn!(
                                                error = %e,
                                                "[BinanceWsApiOrderWs] WS read error"
                                            );
                                            break;
                                        }
                                        None => {
                                            tracing::debug!("[BinanceWsApiOrderWs] WS stream ended");
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                                _ = ws_ping_tick.tick() => {
                                    if !running.load(Ordering::Relaxed) {
                                        break;
                                    }
                                    if write.send(Message::Ping(Vec::new().into())).await.is_err() {
                                        tracing::warn!("[BinanceWsApiOrderWs] Failed to send WS ping");
                                        break;
                                    }
                                }
                                _ = user_ping_tick.tick() => {
                                    if !running.load(Ordering::Relaxed) {
                                        break;
                                    }
                                    // 发送 userDataStream.ping 保活用户数据流
                                    let ping_id = request_id.fetch_add(1, Ordering::Relaxed);
                                    let ping_msg = serde_json::json!({
                                        "id": ping_id,
                                        "method": "userDataStream.ping",
                                        "params": {}
                                    });
                                    let ping_text = match serde_json::to_string(&ping_msg) {
                                        Ok(s) => s,
                                        Err(e) => {
                                            tracing::error!(
                                                error = %e,
                                                "[BinanceWsApiOrderWs] Failed to serialize ping_msg — skipping send"
                                            );
                                            continue;
                                        }
                                    };
                                    if write
                                        .send(Message::Text(ping_text.into()))
                                        .await
                                        .is_err()
                                    {
                                        tracing::warn!("[BinanceWsApiOrderWs] Failed to send userDataStream.ping");
                                        break;
                                    }
                                    tracing::debug!(
                                        "[BinanceWsApiOrderWs] Sent userDataStream.ping (id={})",
                                        ping_id
                                    );
                                }
                            }
                        }

                        let _ = event_tx
                            .send(WsFeedEvent::ConnectionChanged { connected: false })
                            .await;
                        // 如果发送失败，receiver 已关闭，停止重连
                        if !running.load(Ordering::Relaxed) {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "[BinanceWsApiOrderWs] Failed to connect, retrying in {}s",
                            reconnect_delay
                        );
                        if event_tx
                            .send(WsFeedEvent::ConnectionChanged { connected: false })
                            .await
                            .is_err()
                        {
                            tracing::warn!("[BinanceWsApiOrderWs] Event channel closed, stopping");
                            running.store(false, Ordering::Relaxed);
                            break;
                        }
                        tokio::time::sleep(Duration::from_secs(reconnect_delay)).await;
                        reconnect_delay = (reconnect_delay * 2).min(max_reconnect_delay_secs);
                    }
                }
            }

            tracing::debug!("[BinanceWsApiOrderWs] Background task exited");
        });
    }

    /// 停止 WS 连接。
    ///
    /// 设置 `running=false`，后台 task 在下一次 tick 或消息处理时退出。
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

/// 处理收到的文本消息，转发订单事件。
///
/// 返回 `true` 表示事件通道已关闭，调用方应停止后台 task。
async fn handle_text_message(text: &str, event_tx: &mpsc::Sender<WsFeedEvent>) -> bool {
    // WebSocket API 响应有两种：
    // 1. 请求响应: {"id":..., "status":200, "result":..., "rateLimits":[...]}
    // 2. 用户数据事件: {"e":"executionReport", ...} / {"e":"outboundAccountPosition", ...}
    //
    // 只关心用户数据事件，请求响应用 debug 日志记录即可。
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
        if json.get("id").is_some() && json.get("status").is_some() {
            // 请求响应
            tracing::debug!(
                "[BinanceWsApiOrderWs] Request response: id={} status={}",
                json.get("id").and_then(|v| v.as_u64()).unwrap_or(0),
                json.get("status").and_then(|v| v.as_u64()).unwrap_or(0),
            );
            return false;
        }
    }

    // 尝试解析为用户数据事件
    if let Ok(bmsg) = serde_json::from_str::<BinanceOrderMessage>(text) {
        let event_type = bmsg
            .event_type()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        if event_type == "executionReport" || event_type == "ORDER_TRADE_UPDATE" {
            if let Some(event) = bmsg.to_ws_feed_event() {
                tracing::debug!(
                    "[BinanceWsApiOrderWs] {} event received, forwarding",
                    event_type
                );
                if event_tx.send(event).await.is_err() {
                    tracing::warn!("[BinanceWsApiOrderWs] Event channel closed, stopping");
                    return true;
                }
            }
        } else if event_type == "outboundAccountPosition" || event_type == "ACCOUNT_UPDATE" {
            tracing::debug!(
                "[BinanceWsApiOrderWs] {} event received (balance/position update)",
                event_type
            );
        } else if event_type == "listStatus" {
            tracing::debug!("[BinanceWsApiOrderWs] listStatus event received (OCO order)");
        } else {
            tracing::trace!("[BinanceWsApiOrderWs] Ignoring event type: {}", event_type);
        }
    } else {
        tracing::trace!(
            "[BinanceWsApiOrderWs] Unparseable message: {}",
            &text[..text.len().min(200)]
        );
    }
    false
}

/// 等待指定 id 的响应，返回 true 表示 status=200
async fn wait_for_response(
    read: &mut futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    expected_id: u64,
    timeout: Duration,
) -> Result<bool, ExchangeError> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(ExchangeError::Internal(format!(
                "Timeout waiting for response id={}",
                expected_id
            )));
        }

        tokio::select! {
            msg = tokio::time::timeout(remaining, read.next()) => {
                match msg {
                    Ok(Some(Ok(Message::Text(text)))) => {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                            let id = json.get("id").and_then(|v| v.as_u64());
                            let status = json.get("status").and_then(|v| v.as_u64());
                            if id == Some(expected_id) {
                                if let Some(s) = status {
                                    return Ok(s == 200);
                                }
                                return Err(ExchangeError::Internal(format!(
                                    "Response id={} missing status field",
                                    expected_id
                                )));
                            }
                            // 不是等待的响应，可能是用户数据事件，忽略（后续主循环会处理）
                        }
                    }
                    Ok(Some(Ok(_))) => { /* ignore non-text */ }
                    Ok(Some(Err(e))) => return Err(ExchangeError::Internal(format!("WS read error: {}", e))),
                    Ok(None) => return Err(ExchangeError::Internal("WS stream ended".to_string())),
                    Err(_) => return Err(ExchangeError::Internal(format!(
                        "Timeout waiting for response id={}",
                        expected_id
                    ))),
                }
            }
        }
    }
}

/// 构造 session.logon 请求
///
/// 签名方式：把 params（除 signature 外）按 key 字典序排列，
/// 拼成 `key1=value1&key2=value2`，用 Ed25519 签名后 base64 编码。
pub(crate) fn build_session_logon_request(
    signer: &BinanceEd25519Signer,
    id: u64,
) -> Result<serde_json::Value, ExchangeError> {
    let timestamp = chrono::Utc::now().timestamp_millis();
    let api_key = signer.api_key();

    // 按 key 字典序排列：apiKey, recvWindow, timestamp
    // 注意：币安 WebSocket API session.logon 的 params 必须按字典序签名
    let params_for_signing = [
        ("apiKey", api_key.to_string()),
        ("recvWindow", "5000".to_string()),
        ("timestamp", timestamp.to_string()),
    ];

    let query_string = params_for_signing
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&");

    let signature = signer.sign_message(&query_string);

    Ok(serde_json::json!({
        "id": id,
        "method": "session.logon",
        "params": {
            "apiKey": api_key,
            "recvWindow": 5000,
            "timestamp": timestamp,
            "signature": signature,
        }
    }))
}
