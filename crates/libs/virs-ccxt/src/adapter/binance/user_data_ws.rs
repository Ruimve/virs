use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite};

// Re-export for convenience
pub use virs_types::WsFeedEvent;
use virs_types::{OrderStatus, PositionSide};

// ============================================================
// Binance User Data Stream 消息格式
// ============================================================

/// Binance WS 推送两种格式（与 kline 一致）：
/// 1. 单流格式: {"e":"executionReport", ...}
/// 2. 组合流格式: {"stream":"<listenKey>@executionReport", "data":{...}}
#[derive(Debug, Clone, Deserialize)]
pub struct BinanceOrderMessage {
    #[allow(dead_code)]
    pub(crate) stream: Option<String>,
    /// 组合流格式
    pub(crate) data: Option<BinanceExecutionReport>,
    /// 单流格式
    #[serde(rename = "e")]
    pub(crate) event_type_flat: Option<String>,
    /// 单流格式的完整字段
    #[serde(rename = "E")]
    event_time_flat: Option<i64>,
    #[serde(rename = "o")]
    order_flat: Option<ExecutionReportInner>,
}

impl BinanceOrderMessage {
    /// 解析单流或组合流格式的 executionReport
    ///
    /// 对外暴露为 pub，供 ws_api.rs（WebSocket API 客户端）复用
    pub fn into_execution_report(self) -> Option<BinanceExecutionReport> {
        if let Some(data) = self.data {
            Some(data)
        } else if self.event_type_flat.as_deref() == Some("executionReport") {
            self.order_flat.map(|order| BinanceExecutionReport {
                event_type: self.event_type_flat.unwrap_or_else(|| {
                    tracing::error!("order_ws event_type_flat is None — data corruption");
                    "unknown".to_string()
                }),
                event_time: self.event_time_flat.unwrap_or(0),
                order,
            })
        } else {
            None
        }
    }

    /// 返回事件类型（用于判断 executionReport / ORDER_TRADE_UPDATE / ACCOUNT_UPDATE）
    pub fn event_type(&self) -> Option<&str> {
        self.event_type_flat
            .as_deref()
            .or_else(|| self.data.as_ref().map(|d| d.event_type.as_str()))
    }

    /// 转换为 WsFeedEvent（消耗 self）
    pub fn to_ws_feed_event(self) -> Option<WsFeedEvent> {
        // 优先处理 ORDER_TRADE_UPDATE（合约）
        if let Some(et) = self.event_type_flat.as_deref() {
            if et == "ORDER_TRADE_UPDATE" {
                // ORDER_TRADE_UPDATE 的订单数据在 "o" 字段
                if let Some(order) = self.order_flat {
                    return order.to_ws_feed_event();
                }
            }
        }
        // 处理 executionReport（现货）
        if let Some(report) = self.into_execution_report() {
            return report.order.to_ws_feed_event();
        }
        None
    }
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct BinanceExecutionReport {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: i64,
    #[serde(rename = "o")]
    pub order: ExecutionReportInner,
}

/// Binance executionReport 中的订单数据
/// 文档: https://binance-docs.github.io/apidocs/futures/en/#event-order-update
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ExecutionReportInner {
    /// 订单符号
    #[serde(rename = "s")]
    pub(crate) symbol: String,
    /// 客户端订单 ID
    #[serde(rename = "c")]
    pub(crate) client_order_id: String,
    /// 侧: BUY / SELL
    #[serde(rename = "S")]
    pub(crate) side: String,
    /// 订单类型
    #[serde(rename = "o")]
    pub(crate) order_type: String,
    /// 订单状态
    #[serde(rename = "X")]
    pub(crate) status: String,
    /// 订单 ID
    #[serde(rename = "i")]
    pub(crate) order_id: i64,
    /// 原始订单数量
    #[serde(rename = "q")]
    pub(crate) orig_qty: String,
    /// 已填充数量
    #[serde(rename = "z")]
    pub(crate) filled_qty: String,
    /// 剩余数量
    #[serde(rename = "Q")]
    pub(crate) remaining_qty: Option<String>,
    /// 成交价格（最后一笔成交价）
    #[serde(rename = "L")]
    pub(crate) last_fill_price: String,
    /// 累计成交均价（仅永续合约 ORDER_TRADE_UPDATE 提供）
    #[serde(rename = "ap")]
    pub(crate) avg_fill_price: Option<String>,
    /// 成交数量
    #[serde(rename = "l")]
    pub(crate) last_fill_qty: String,
    /// 手续费
    #[serde(rename = "n")]
    pub(crate) commission: String,
    /// 手续费资产
    #[serde(rename = "N")]
    pub(crate) commission_asset: String,
    /// 订单创建时间
    #[serde(rename = "T")]
    pub(crate) trade_time: i64,
    /// 是否是 reduce-only
    #[serde(rename = "R")]
    pub(crate) is_reduce_only: bool,
    /// 工作类型
    #[serde(rename = "w")]
    pub(crate) working_type: String,
    /// 持仓方向: LONG / SHORT / BOTH（双向持仓模式下区分多空持仓）
    #[serde(rename = "ps")]
    pub(crate) position_side: Option<String>,
}

impl ExecutionReportInner {
    /// 将 Binance 订单状态映射为 Position Engine 的 OrderStatus
    pub(crate) fn to_order_status(&self) -> Option<OrderStatus> {
        match self.status.as_str() {
            "NEW" => Some(OrderStatus::Open),
            "PARTIALLY_FILLED" => Some(OrderStatus::PartiallyFilled),
            "FILLED" => Some(OrderStatus::Filled),
            "CANCELED" => Some(OrderStatus::Canceled),
            "EXPIRED" => Some(OrderStatus::Canceled),
            "EXPIRED_IN_MATCH" => Some(OrderStatus::Canceled),
            "REJECTED" => Some(OrderStatus::Failed),
            _ => None,
        }
    }

    /// 转换为 WsFeedEvent::OrderUpdate
    ///
    /// 关键数值字段（filled/amount/price/commission）解析失败时返回 None，
    /// 跳过该事件而非传播 0.0，避免订单状态判断错误和 PnL 计算偏差。
    pub fn to_ws_feed_event(&self) -> Option<WsFeedEvent> {
        let status = self.to_order_status()?;

        let position_side = self
            .position_side
            .as_ref()
            .and_then(|ps| match ps.as_str() {
                "LONG" => Some(PositionSide::Long),
                "SHORT" => Some(PositionSide::Short),
                _ => None,
            });

        let filled = self.filled_qty.parse::<f64>().unwrap_or_else(|e| {
            tracing::error!(
                filled_qty = %self.filled_qty,
                error = %e,
                "Failed to parse filled_qty in order_ws — skipping event to avoid 0.0 propagation"
            );
            f64::NAN
        });
        if filled.is_nan() {
            return None;
        }

        let amount = self.orig_qty.parse::<f64>().unwrap_or_else(|e| {
            tracing::error!(
                orig_qty = %self.orig_qty,
                error = %e,
                "Failed to parse orig_qty in order_ws — skipping event to avoid 0.0 propagation"
            );
            f64::NAN
        });
        if amount.is_nan() {
            return None;
        }

        let remaining = self
            .remaining_qty
            .as_ref()
            .and_then(|q| q.parse().ok())
            .unwrap_or_else(|| (amount - filled).max(0.0));

        let price = self
            .avg_fill_price
            .as_ref()
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|&p| p > 0.0)
            .unwrap_or_else(|| {
                match self.last_fill_price.parse::<f64>() {
                    Ok(p) if p > 0.0 => p,
                    Ok(_) => {
                        tracing::warn!(
                            last_fill_price = %self.last_fill_price,
                            "last_fill_price is 0.0 in order_ws — using 0.0 (order may not be filled yet)"
                        );
                        0.0
                    }
                    Err(e) => {
                        tracing::error!(
                            last_fill_price = %self.last_fill_price,
                            error = %e,
                            "Failed to parse last_fill_price in order_ws — skipping event to avoid 0.0 price propagation"
                        );
                        return f64::NAN;
                    }
                }
            });
        if price.is_nan() {
            return None;
        }

        let commission = match self.commission.parse::<f64>() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(
                    commission = %self.commission,
                    error = %e,
                    "Failed to parse commission in order_ws — skipping event to avoid 0.0 propagation"
                );
                return None;
            }
        };

        Some(WsFeedEvent::OrderUpdate {
            exchange_order_id: self.order_id.to_string(),
            symbol: self.symbol.clone(),
            status,
            filled,
            remaining,
            price,
            amount,
            commission,
            timestamp: DateTime::from_timestamp_millis(self.trade_time).unwrap_or_else(Utc::now),
            position_side,
        })
    }
}

// ============================================================
// UserDataWs: 订单 WebSocket 客户端
// ============================================================

/// Binance User Data Stream 订单推送客户端
///
/// 连接到 Binance 的 User Data Stream（需要 listenKey），
/// 接收 executionReport 事件并转换为 WsFeedEvent。
///
/// 连接管理参考 KlineWs：
/// - 指数退避重连
/// - Ping/Pong 心跳
/// - 最大生命周期（23 小时）
pub struct UserDataWs {
    /// WS URL（包含 listenKey）
    pub(crate) ws_url: String,
    /// 基础 URL（不含 listenKey，用于重连时拼接新 listenKey）
    pub(crate) base_url: String,
    /// URL 格式：true=query 参数形态（?listenKey=），false=path 形态（/<listenKey>）
    /// 永续合约 /private 路由使用 query 形态，现货 /ws 路由使用 path 形态
    use_query_params: bool,
    reconnect_delay_secs: u64,
    max_reconnect_delay_secs: u64,
    ws_ping_interval_secs: u64,
    ws_max_lifetime_secs: u64,
    running: Arc<AtomicBool>,
}

impl UserDataWs {
    /// 创建新的订单 WS 客户端（path 形态 URL）
    ///
    /// # 参数
    /// - `base_url`: WS 基础 URL（如 `wss://stream.binance.com/ws`）
    /// - `listen_key`: Binance User Data Stream 的 listenKey
    pub fn new(base_url: String, listen_key: String) -> Self {
        let ws_url = format!("{}/{}", base_url.trim_end_matches('/'), listen_key);
        Self {
            ws_url,
            base_url,
            use_query_params: false,
            reconnect_delay_secs: 1,
            max_reconnect_delay_secs: 60,
            ws_ping_interval_secs: 30,
            ws_max_lifetime_secs: 23 * 3600,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 创建永续合约订单 WS 客户端
    ///
    /// 2026-04-23 起币安将用户数据流切流至 /private 路由，
    /// 新 URL 使用 query 形态 `wss://fstream.binance.com/private/ws?listenKey=<key>`
    /// （官方迁移公告示例格式）。
    pub fn new_perpetual(listen_key: String) -> Self {
        let base_url = "wss://fstream.binance.com/private/ws".to_string();
        let ws_url = format!("{}?listenKey={}", base_url, listen_key);
        Self {
            ws_url,
            base_url,
            use_query_params: true,
            reconnect_delay_secs: 1,
            max_reconnect_delay_secs: 60,
            ws_ping_interval_secs: 30,
            ws_max_lifetime_secs: 23 * 3600,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 创建现货订单 WS 客户端
    pub fn new_spot(listen_key: String) -> Self {
        Self::new("wss://stream.binance.com/ws".to_string(), listen_key)
    }

    /// 更新 listenKey（重连时使用）
    pub fn update_listen_key(&mut self, listen_key: String) {
        if self.use_query_params {
            self.ws_url = format!("{}?listenKey={}", self.base_url.trim_end_matches('/'), listen_key);
        } else {
            self.ws_url = format!("{}/{}", self.base_url.trim_end_matches('/'), listen_key);
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// 返回 running flag 的引用，供外部 keepalive task 检测 WS 生命周期。
    ///
    /// WS 后台 task 退出时会将此 flag 设为 false，keepalive task 应定期检测并退出。
    pub fn running_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.running)
    }

    /// 停止 WS 连接
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }

    /// 启动 WS 连接，将订单事件发送到 event_tx
    ///
    /// 返回后立即返回，WS 连接在后台 tokio task 中运行。
    /// 当 WS 断开时发送 ConnectionChanged { connected: false }，
    /// 重连成功后发送 ConnectionChanged { connected: true }。
    pub async fn start(&mut self, event_tx: mpsc::Sender<WsFeedEvent>) {
        if self.running.load(Ordering::Relaxed) {
            return;
        }
        self.running.store(true, Ordering::Relaxed);

        let ws_url = self.ws_url.clone();
        let running = self.running.clone();
        let reconnect_delay_secs = self.reconnect_delay_secs;
        let max_reconnect_delay_secs = self.max_reconnect_delay_secs;
        let ws_ping_interval_secs = self.ws_ping_interval_secs;
        let ws_max_lifetime_secs = self.ws_max_lifetime_secs;

        tokio::spawn(async move {
            let mut reconnect_delay = reconnect_delay_secs;

            while running.load(Ordering::Relaxed) {
                let connect_start = tokio::time::Instant::now();

                match connect_async(&ws_url).await {
                    Ok((ws_stream, _)) => {
                        reconnect_delay = reconnect_delay_secs;

                        // 发送连接恢复事件
                        if event_tx
                            .send(WsFeedEvent::ConnectionChanged { connected: true })
                            .await
                            .is_err()
                        {
                            tracing::warn!(
                                "[UserDataWs] Event channel closed on connect, stopping"
                            );
                            running.store(false, Ordering::Relaxed);
                            break;
                        }

                        let (mut write, mut read) = ws_stream.split();

                        let ping_interval = Duration::from_secs(ws_ping_interval_secs);
                        let mut ping_tick = tokio::time::interval(ping_interval);
                        let max_lifetime = Duration::from_secs(ws_max_lifetime_secs);
                        let mut running_check = tokio::time::interval(Duration::from_secs(1));

                        loop {
                            if !running.load(Ordering::Relaxed) {
                                break;
                            }

                            if connect_start.elapsed() > max_lifetime {
                                break;
                            }

                            tokio::select! {
                                msg = read.next() => {
                                    match msg {
                                        Some(Ok(tungstenite::Message::Text(text))) => {
                                            if let Ok(bmsg) = serde_json::from_str::<BinanceOrderMessage>(&text) {
                                                let event_type = bmsg
                                                    .event_type()
                                                    .map(|s| s.to_string())
                                                    .unwrap_or_else(|| "unknown".to_string());

                                                // to_ws_feed_event() 同时处理 executionReport（现货）
                                                // 和 ORDER_TRADE_UPDATE（合约），避免单流格式下
                                                // into_execution_report() 丢弃合约事件。
                                                if let Some(event) = bmsg.to_ws_feed_event() {
                                                    if event_tx.send(event).await.is_err() {
                                                        tracing::warn!("[UserDataWs] Event channel closed, stopping");
                                                        running.store(false, Ordering::Relaxed);
                                                        return;
                                                    }
                                                } else {
                                                    // 非订单事件：ACCOUNT_UPDATE / listenKeyExpired /
                                                    // serverShutdown / MARGIN_CALL / 订阅确认等
                                                    match event_type.as_str() {
                                                        "ACCOUNT_UPDATE" => {
                                                        }
                                                        "listenKeyExpired" => {
                                                            tracing::warn!(
                                                                "[UserDataWs] listenKey expired, will reconnect"
                                                            );
                                                            break;
                                                        }
                                                        "serverShutdown" => {
                                                            break;
                                                        }
                                                        _ => {
                                                            tracing::trace!(
                                                                "[UserDataWs] Ignoring event type: {}",
                                                                event_type
                                                            );
                                                        }
                                                    }
                                                }
                                            } else {
                                                tracing::warn!(
                                                    "[UserDataWs] Failed to parse WS message: {}",
                                                    &text[..text.len().min(200)]
                                                );
                                            }
                                        }
                                        Some(Ok(tungstenite::Message::Ping(data))) => {
                                            let _ = write.send(tungstenite::Message::Pong(data)).await;
                                        }
                                        Some(Ok(tungstenite::Message::Close(_))) => {
                                            tracing::warn!("[UserDataWs] Server closed connection");
                                            break;
                                        }
                                        Some(Err(e)) => {
                                            tracing::error!("[UserDataWs] Read error: {}", e);
                                            break;
                                        }
                                        None => {
                                            tracing::warn!("[UserDataWs] Stream ended");
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                                _ = ping_tick.tick() => {
                                    let ping = tungstenite::Message::Ping(vec![].into());
                                    if write.send(ping).await.is_err() {
                                        tracing::warn!("[UserDataWs] Ping failed, reconnecting...");
                                        break;
                                    }
                                }
                                _ = running_check.tick() => {
                                    if !running.load(Ordering::Relaxed) {
                                        let _ = write.send(tungstenite::Message::Close(None)).await;
                                        break;
                                    }
                                }
                            }
                        }

                        // 连接断开，发送断连事件
                        if event_tx
                            .send(WsFeedEvent::ConnectionChanged { connected: false })
                            .await
                            .is_err()
                        {
                            tracing::warn!(
                                "[UserDataWs] Event channel closed on disconnect, stopping"
                            );
                            running.store(false, Ordering::Relaxed);
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::error!("[UserDataWs] Connection failed: {}", e);
                    }
                }

                if !running.load(Ordering::Relaxed) {
                    break;
                }

                let jitter = (reconnect_delay as f64 * 0.1 * (2.0 * rand::random::<f64>() - 1.0)) as i64;
                let delay = (reconnect_delay as i64 + jitter).max(1) as u64;
                tokio::time::sleep(Duration::from_secs(delay)).await;
                reconnect_delay = (reconnect_delay * 2).min(max_reconnect_delay_secs);
            }

            running.store(false, Ordering::Relaxed);
        });
    }
}
