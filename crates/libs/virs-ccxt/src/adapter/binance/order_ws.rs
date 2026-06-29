use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite};

// Re-export for convenience
pub use crate::ws_types::WsFeedEvent;
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
    stream: Option<String>,
    /// 组合流格式
    data: Option<BinanceExecutionReport>,
    /// 单流格式
    #[serde(rename = "e")]
    event_type_flat: Option<String>,
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
                event_type: self.event_type_flat.unwrap(),
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
    symbol: String,
    /// 客户端订单 ID
    #[serde(rename = "c")]
    client_order_id: String,
    /// 侧: BUY / SELL
    #[serde(rename = "S")]
    side: String,
    /// 订单类型
    #[serde(rename = "o")]
    order_type: String,
    /// 订单状态
    #[serde(rename = "X")]
    status: String,
    /// 订单 ID
    #[serde(rename = "i")]
    order_id: i64,
    /// 原始订单数量
    #[serde(rename = "q")]
    orig_qty: String,
    /// 已填充数量
    #[serde(rename = "z")]
    filled_qty: String,
    /// 剩余数量
    #[serde(rename = "Q")]
    remaining_qty: Option<String>,
    /// 成交价格（最后一笔成交价）
    #[serde(rename = "L")]
    last_fill_price: String,
    /// 累计成交均价（仅永续合约 ORDER_TRADE_UPDATE 提供）
    #[serde(rename = "ap")]
    avg_fill_price: Option<String>,
    /// 成交数量
    #[serde(rename = "l")]
    last_fill_qty: String,
    /// 手续费
    #[serde(rename = "n")]
    commission: String,
    /// 手续费资产
    #[serde(rename = "N")]
    commission_asset: String,
    /// 订单创建时间
    #[serde(rename = "T")]
    trade_time: i64,
    /// 是否是 reduce-only
    #[serde(rename = "R")]
    is_reduce_only: bool,
    /// 工作类型
    #[serde(rename = "w")]
    working_type: String,
    /// 持仓方向: LONG / SHORT / BOTH（双向持仓模式下区分多空持仓）
    #[serde(rename = "ps")]
    position_side: Option<String>,
}

impl ExecutionReportInner {
    /// 将 Binance 订单状态映射为 Position Engine 的 OrderStatus
    fn to_order_status(&self) -> Option<OrderStatus> {
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

        Some(WsFeedEvent::OrderUpdate {
            exchange_order_id: self.order_id.to_string(),
            symbol: self.symbol.clone(),
            status,
            filled: self.filled_qty.parse().unwrap_or(0.0),
            remaining: self
                .remaining_qty
                .as_ref()
                .and_then(|q| q.parse().ok())
                .unwrap_or_else(|| {
                    let orig = self.orig_qty.parse::<f64>().unwrap_or(0.0);
                    let filled = self.filled_qty.parse::<f64>().unwrap_or(0.0);
                    (orig - filled).max(0.0)
                }),
            price: self
                .avg_fill_price
                .as_ref()
                .and_then(|s| s.parse::<f64>().ok())
                .filter(|&p| p > 0.0)
                .unwrap_or_else(|| self.last_fill_price.parse().unwrap_or(0.0)),
            amount: self.orig_qty.parse().unwrap_or(0.0),
            commission: self.commission.parse().unwrap_or(0.0),
            timestamp: DateTime::from_timestamp_millis(self.trade_time).unwrap_or_else(Utc::now),
            position_side,
        })
    }
}

// ============================================================
// BinanceOrderWs: 订单 WebSocket 客户端
// ============================================================

/// Binance User Data Stream 订单推送客户端
///
/// 连接到 Binance 的 User Data Stream（需要 listenKey），
/// 接收 executionReport 事件并转换为 WsFeedEvent。
///
/// 连接管理参考 BinanceKlineWs：
/// - 指数退避重连
/// - Ping/Pong 心跳
/// - 最大生命周期（23 小时）
pub struct BinanceOrderWs {
    /// WS URL（包含 listenKey）
    ws_url: String,
    /// 基础 URL（不含 listenKey，用于重连时拼接新 listenKey）
    base_url: String,
    reconnect_delay_secs: u64,
    max_reconnect_delay_secs: u64,
    ws_ping_interval_secs: u64,
    ws_max_lifetime_secs: u64,
    running: Arc<AtomicBool>,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl BinanceOrderWs {
    /// 创建新的订单 WS 客户端
    ///
    /// # 参数
    /// - `base_url`: WS 基础 URL（如 `wss://fstream.binance.com/ws`）
    /// - `listen_key`: Binance User Data Stream 的 listenKey
    pub fn new(base_url: String, listen_key: String) -> Self {
        let ws_url = format!("{}/{}", base_url.trim_end_matches('/'), listen_key);
        Self {
            ws_url,
            base_url,
            reconnect_delay_secs: 1,
            max_reconnect_delay_secs: 60,
            ws_ping_interval_secs: 30,
            ws_max_lifetime_secs: 23 * 3600,
            running: Arc::new(AtomicBool::new(false)),
            shutdown_tx: None,
        }
    }

    /// 创建永续合约订单 WS 客户端
    ///
    /// 2026-04-23 起币安将用户数据流切流至 /private 路由。
    /// 旧 URL `wss://fstream.binance.com/ws/<listenKey>` 已无法连接，
    /// 新 URL `wss://fstream.binance.com/private/ws/<listenKey>`（单流模式，协议不变）。
    pub fn new_perpetual(listen_key: String) -> Self {
        Self::new(
            "wss://fstream.binance.com/private/ws".to_string(),
            listen_key,
        )
    }

    /// 创建现货订单 WS 客户端
    pub fn new_spot(listen_key: String) -> Self {
        Self::new("wss://stream.binance.com/ws".to_string(), listen_key)
    }

    /// 更新 listenKey（重连时使用）
    pub fn update_listen_key(&mut self, listen_key: String) {
        self.ws_url = format!("{}/{}", self.base_url.trim_end_matches('/'), listen_key);
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

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx);

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

                tracing::debug!("[BinanceOrderWs] Connecting to {}...", ws_url);

                match connect_async(&ws_url).await {
                    Ok((ws_stream, _)) => {
                        tracing::debug!("[BinanceOrderWs] Connected to {}", ws_url);
                        reconnect_delay = reconnect_delay_secs;

                        // 发送连接恢复事件
                        let _ = event_tx
                            .send(WsFeedEvent::ConnectionChanged { connected: true })
                            .await;

                        let (mut write, mut read) = ws_stream.split();

                        let ping_interval = Duration::from_secs(ws_ping_interval_secs);
                        let mut ping_tick = tokio::time::interval(ping_interval);
                        let max_lifetime = Duration::from_secs(ws_max_lifetime_secs);

                        loop {
                            if !running.load(Ordering::Relaxed) {
                                break;
                            }

                            if connect_start.elapsed() > max_lifetime {
                                tracing::debug!(
                                    "[BinanceOrderWs] Max lifetime reached, reconnecting..."
                                );
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
                                                    let status_str = match &event {
                                                        WsFeedEvent::OrderUpdate { status, .. } => format!("{:?}", status),
                                                        _ => "unknown".to_string(),
                                                    };
                                                    tracing::debug!(
                                                        "[BinanceOrderWs] {} -> OrderUpdate status={} forwarded",
                                                        event_type, status_str
                                                    );
                                                    if event_tx.send(event).await.is_err() {
                                                        tracing::warn!("[BinanceOrderWs] Event channel closed, stopping");
                                                        running.store(false, Ordering::Relaxed);
                                                        return;
                                                    }
                                                } else {
                                                    // 非订单事件：ACCOUNT_UPDATE / listenKeyExpired /
                                                    // serverShutdown / MARGIN_CALL / 订阅确认等
                                                    match event_type.as_str() {
                                                        "ACCOUNT_UPDATE" => {
                                                            tracing::debug!(
                                                                "[BinanceOrderWs] ACCOUNT_UPDATE received (balance/position change)"
                                                            );
                                                        }
                                                        "listenKeyExpired" => {
                                                            tracing::warn!(
                                                                "[BinanceOrderWs] listenKey expired, will reconnect"
                                                            );
                                                            break;
                                                        }
                                                        "serverShutdown" => {
                                                            tracing::info!(
                                                                "[BinanceOrderWs] serverShutdown received, server will disconnect soon; reconnecting"
                                                            );
                                                            break;
                                                        }
                                                        _ => {
                                                            tracing::trace!(
                                                                "[BinanceOrderWs] Ignoring event type: {}",
                                                                event_type
                                                            );
                                                        }
                                                    }
                                                }
                                            } else {
                                                tracing::warn!(
                                                    "[BinanceOrderWs] Failed to parse WS message: {}",
                                                    &text[..text.len().min(200)]
                                                );
                                            }
                                        }
                                        Some(Ok(tungstenite::Message::Ping(data))) => {
                                            let _ = write.send(tungstenite::Message::Pong(data)).await;
                                        }
                                        Some(Ok(tungstenite::Message::Close(_))) => {
                                            tracing::warn!("[BinanceOrderWs] Server closed connection");
                                            break;
                                        }
                                        Some(Err(e)) => {
                                            tracing::error!("[BinanceOrderWs] Read error: {}", e);
                                            break;
                                        }
                                        None => {
                                            tracing::warn!("[BinanceOrderWs] Stream ended");
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                                _ = ping_tick.tick() => {
                                    let ping = tungstenite::Message::Ping(vec![].into());
                                    if write.send(ping).await.is_err() {
                                        tracing::warn!("[BinanceOrderWs] Ping failed, reconnecting...");
                                        break;
                                    }
                                }
                                _ = shutdown_rx.recv() => {
                                    tracing::debug!("[BinanceOrderWs] Shutdown requested");
                                    let _ = write.send(tungstenite::Message::Close(None)).await;
                                    running.store(false, Ordering::Relaxed);
                                    return;
                                }
                            }
                        }

                        // 连接断开，发送断连事件
                        let _ = event_tx
                            .send(WsFeedEvent::ConnectionChanged { connected: false })
                            .await;
                    }
                    Err(e) => {
                        tracing::error!("[BinanceOrderWs] Connection failed: {}", e);
                    }
                }

                if !running.load(Ordering::Relaxed) {
                    break;
                }

                tracing::debug!("[BinanceOrderWs] Reconnecting in {}s...", reconnect_delay);
                tokio::time::sleep(Duration::from_secs(reconnect_delay)).await;
                reconnect_delay = (reconnect_delay * 2).min(max_reconnect_delay_secs);
            }

            running.store(false, Ordering::Relaxed);
            tracing::debug!("[BinanceOrderWs] Worker exited");
        });
    }

    /// 停止 WS 连接
    pub async fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // 消息解析 (5 tests)
    // ============================================================

    #[test]
    fn test_parse_execution_report_basic() {
        let json = r#"{
            "e": "executionReport",
            "E": 1713900000000,
            "o": {
                "s": "BTCUSDT",
                "c": "client_123",
                "S": "BUY",
                "o": "LIMIT",
                "X": "FILLED",
                "i": 123456789,
                "q": "1.000",
                "z": "1.000",
                "Q": "0.000",
                "L": "65000.00",
                "l": "1.000",
                "n": "0.065",
                "N": "USDT",
                "T": 1713900000123,
                "R": false,
                "w": "CONTRACT_PRICE"
            }
        }"#;

        let msg: BinanceOrderMessage = serde_json::from_str(json).unwrap();
        assert!(msg.stream.is_none());
        assert!(msg.data.is_none());
        assert_eq!(msg.event_type_flat.as_deref(), Some("executionReport"));

        let report = msg.into_execution_report().unwrap();
        assert_eq!(report.event_type, "executionReport");
        assert_eq!(report.order.symbol, "BTCUSDT");
        assert_eq!(report.order.status, "FILLED");
        assert_eq!(report.order.order_id, 123456789);
    }

    #[test]
    fn test_parse_execution_report_combined_stream() {
        let json = r#"{
            "stream": "listenKey@executionReport",
            "data": {
                "e": "executionReport",
                "E": 1713900000000,
                "o": {
                    "s": "ETHUSDT",
                    "c": "client_456",
                    "S": "SELL",
                    "o": "MARKET",
                    "X": "PARTIALLY_FILLED",
                    "i": 987654321,
                    "q": "10.000",
                    "z": "5.000",
                    "Q": "5.000",
                    "L": "3500.00",
                    "l": "5.000",
                    "n": "0.175",
                    "N": "USDT",
                    "T": 1713900000456,
                    "R": true,
                    "w": "MARK_PRICE"
                }
            }
        }"#;

        let msg: BinanceOrderMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.stream.as_deref(), Some("listenKey@executionReport"));
        assert!(msg.data.is_some());

        let report = msg.into_execution_report().unwrap();
        assert_eq!(report.order.symbol, "ETHUSDT");
        assert_eq!(report.order.status, "PARTIALLY_FILLED");
        assert_eq!(report.order.side, "SELL");
        assert!(report.order.is_reduce_only);
    }

    #[test]
    fn test_parse_invalid_json() {
        let result: Result<BinanceOrderMessage, _> = serde_json::from_str("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_non_order_event() {
        // ACCOUNT_UPDATE 事件没有 "o" 字段
        let json = r#"{
            "e": "ACCOUNT_UPDATE",
            "E": 1713900000000,
            "T": 1713900000000
        }"#;

        let msg: BinanceOrderMessage = serde_json::from_str(json).unwrap();
        let report = msg.into_execution_report();
        // 没有订单数据，应该返回 None
        assert!(report.is_none());
    }

    #[test]
    fn test_parse_listen_key_expired() {
        // listenKey 过期事件
        let json = r#"{
            "e": "listenKeyExpired",
            "E": 1713900000000
        }"#;

        let msg: BinanceOrderMessage = serde_json::from_str(json).unwrap();
        let report = msg.into_execution_report();
        assert!(report.is_none());
    }

    #[test]
    fn test_parse_order_trade_update_single_stream() {
        // 永续合约 ORDER_TRADE_UPDATE 单流格式
        // 之前 into_execution_report() 对单流 ORDER_TRADE_UPDATE 返回 None，
        // 导致合约订单更新被静默丢弃。to_ws_feed_event() 应正确处理。
        let json = r#"{
            "e": "ORDER_TRADE_UPDATE",
            "E": 1713900000000,
            "T": 1713900000123,
            "o": {
                "s": "BTCUSDT",
                "c": "test_client",
                "S": "SELL",
                "o": "LIMIT",
                "f": "GTC",
                "q": "0.002",
                "p": "65000.00",
                "ap": "65000.00",
                "x": "FILLED",
                "X": "FILLED",
                "i": 123456789,
                "l": "0.002",
                "z": "0.002",
                "L": "65000.00",
                "n": "0.065",
                "N": "USDT",
                "T": 1713900000123,
                "t": 1,
                "R": true,
                "w": "CONTRACT_PRICE",
                "m": false,
                "ps": "SHORT"
            }
        }"#;

        let msg: BinanceOrderMessage = serde_json::from_str(json).unwrap();
        // 单流 ORDER_TRADE_UPDATE 不应被 into_execution_report 处理
        assert!(msg.clone().into_execution_report().is_none());

        // 但 to_ws_feed_event() 应正确转换为 WsFeedEvent
        let event = msg.to_ws_feed_event();
        assert!(event.is_some(), "ORDER_TRADE_UPDATE should produce WsFeedEvent");

        if let Some(WsFeedEvent::OrderUpdate {
            exchange_order_id,
            symbol,
            status,
            filled,
            remaining,
            position_side,
            ..
        }) = event
        {
            assert_eq!(exchange_order_id, "123456789");
            assert_eq!(symbol, "BTCUSDT");
            assert_eq!(status, OrderStatus::Filled);
            assert!((filled - 0.002).abs() < 0.0001);
            assert!((remaining - 0.0).abs() < 0.0001);
            assert_eq!(position_side, Some(PositionSide::Short));
        } else {
            panic!("Expected OrderUpdate event");
        }
    }

    // ============================================================
    // 状态映射 (6 tests)
    // ============================================================

    #[test]
    fn test_order_status_mapping_all_variants() {
        let cases = vec![
            ("NEW", Some(OrderStatus::Open)),
            ("PARTIALLY_FILLED", Some(OrderStatus::PartiallyFilled)),
            ("FILLED", Some(OrderStatus::Filled)),
            ("CANCELED", Some(OrderStatus::Canceled)),
            ("EXPIRED", Some(OrderStatus::Canceled)),
            ("EXPIRED_IN_MATCH", Some(OrderStatus::Canceled)),
            ("REJECTED", Some(OrderStatus::Failed)),
            ("PENDING_CANCEL", None),
        ];

        for (binance_status, expected) in cases {
            let inner = ExecutionReportInner {
                symbol: "BTCUSDT".to_string(),
                client_order_id: "test".to_string(),
                side: "BUY".to_string(),
                order_type: "LIMIT".to_string(),
                status: binance_status.to_string(),
                order_id: 1,
                orig_qty: "1.0".to_string(),
                filled_qty: "0.0".to_string(),
                remaining_qty: Some("1.0".to_string()),
                last_fill_price: "0.0".to_string(),
                avg_fill_price: None,
                last_fill_qty: "0.0".to_string(),
                commission: "0.0".to_string(),
                commission_asset: "USDT".to_string(),
                trade_time: 0,
                is_reduce_only: false,
                working_type: "CONTRACT_PRICE".to_string(),
                position_side: None,
            };
            assert_eq!(
                inner.to_order_status(),
                expected,
                "Binance status '{}' should map to {:?}",
                binance_status,
                expected
            );
        }
    }

    // ============================================================
    // WsFeedEvent 转换 (5 tests)
    // ============================================================

    /// 辅助函数：从 WsFeedEvent 中提取 OrderUpdate 字段
    fn unwrap_order_update(
        event: WsFeedEvent,
    ) -> (String, String, OrderStatus, f64, f64, f64, f64, f64) {
        match event {
            WsFeedEvent::OrderUpdate {
                exchange_order_id,
                symbol,
                status,
                filled,
                remaining,
                price,
                amount,
                commission,
                ..
            } => (
                exchange_order_id,
                symbol,
                status,
                filled,
                remaining,
                price,
                amount,
                commission,
            ),
            WsFeedEvent::ConnectionChanged { .. } => {
                panic!("Expected OrderUpdate, got ConnectionChanged")
            }
        }
    }

    #[test]
    fn test_to_ws_feed_event_filled() {
        let inner = make_test_inner("FILLED", "1.0", "1.0", "0.0", "65000.00", "1.0", "0.065");
        let event = inner.to_ws_feed_event().unwrap();
        let (exchange_order_id, symbol, status, filled, remaining, price, amount, commission) =
            unwrap_order_update(event);

        assert_eq!(exchange_order_id, "123456789");
        assert_eq!(symbol, "BTCUSDT");
        assert_eq!(status, OrderStatus::Filled);
        assert!((filled - 1.0).abs() < 0.001);
        assert!((remaining - 0.0).abs() < 0.001);
        assert!((price - 65000.0).abs() < 0.001);
        assert!((amount - 1.0).abs() < 0.001);
        assert!((commission - 0.065).abs() < 0.001);
    }

    #[test]
    fn test_to_ws_feed_event_partially_filled() {
        let inner = make_test_inner(
            "PARTIALLY_FILLED",
            "10.0",
            "5.0",
            "5.0",
            "3500.00",
            "5.0",
            "0.175",
        );
        let event = inner.to_ws_feed_event().unwrap();
        let (_, _, status, filled, remaining, _, _, _) = unwrap_order_update(event);

        assert_eq!(status, OrderStatus::PartiallyFilled);
        assert!((filled - 5.0).abs() < 0.001);
        assert!((remaining - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_to_ws_feed_event_new_order() {
        let inner = make_test_inner("NEW", "1.0", "0.0", "1.0", "0.00", "0.0", "0.0");
        let event = inner.to_ws_feed_event().unwrap();
        let (_, _, status, filled, remaining, _, _, _) = unwrap_order_update(event);

        assert_eq!(status, OrderStatus::Open);
        assert!((filled - 0.0).abs() < 0.001);
        assert!((remaining - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_to_ws_feed_event_unknown_status() {
        let inner = make_test_inner("PENDING_CANCEL", "1.0", "0.0", "1.0", "0.00", "0.0", "0.0");
        let event = inner.to_ws_feed_event();
        assert!(event.is_none(), "Unknown status should return None");
    }

    #[test]
    fn test_to_ws_feed_event_remaining_fallback() {
        // remaining_qty 为 None 时，使用 orig_qty - filled_qty
        let inner = ExecutionReportInner {
            symbol: "BTCUSDT".to_string(),
            client_order_id: "test".to_string(),
            side: "BUY".to_string(),
            order_type: "LIMIT".to_string(),
            status: "PARTIALLY_FILLED".to_string(),
            order_id: 123456789,
            orig_qty: "10.0".to_string(),
            filled_qty: "3.0".to_string(),
            remaining_qty: None, // None -> fallback
            last_fill_price: "65000.00".to_string(),
            avg_fill_price: None,
            last_fill_qty: "3.0".to_string(),
            commission: "0.195".to_string(),
            commission_asset: "USDT".to_string(),
            trade_time: 1713900000123,
            is_reduce_only: false,
            working_type: "CONTRACT_PRICE".to_string(),
            position_side: None,
        };
        let event = inner.to_ws_feed_event().unwrap();
        let (_, _, _, _, remaining, _, _, _) = unwrap_order_update(event);
        assert!((remaining - 7.0).abs() < 0.001, "remaining = 10 - 3 = 7");
    }

    // ============================================================
    // 构造函数和状态 (3 tests)
    // ============================================================

    #[test]
    fn test_new_perpetual() {
        let ws = BinanceOrderWs::new_perpetual("test_listen_key".to_string());
        assert_eq!(ws.ws_url, "wss://fstream.binance.com/private/ws/test_listen_key");
        assert_eq!(ws.base_url, "wss://fstream.binance.com/private/ws");
        assert!(!ws.is_running());
    }

    #[test]
    fn test_new_spot() {
        let ws = BinanceOrderWs::new_spot("my_key".to_string());
        assert_eq!(ws.ws_url, "wss://stream.binance.com/ws/my_key");
        assert!(!ws.is_running());
    }

    #[test]
    fn test_update_listen_key() {
        let mut ws = BinanceOrderWs::new_perpetual("old_key".to_string());
        assert_eq!(ws.ws_url, "wss://fstream.binance.com/private/ws/old_key");

        ws.update_listen_key("new_key".to_string());
        assert_eq!(ws.ws_url, "wss://fstream.binance.com/private/ws/new_key");
    }

    // ============================================================
    // 辅助函数
    // ============================================================

    fn make_test_inner(
        status: &str,
        orig_qty: &str,
        filled_qty: &str,
        remaining_qty: &str,
        last_fill_price: &str,
        last_fill_qty: &str,
        commission: &str,
    ) -> ExecutionReportInner {
        ExecutionReportInner {
            symbol: "BTCUSDT".to_string(),
            client_order_id: "test_client".to_string(),
            side: "BUY".to_string(),
            order_type: "LIMIT".to_string(),
            status: status.to_string(),
            order_id: 123456789,
            orig_qty: orig_qty.to_string(),
            filled_qty: filled_qty.to_string(),
            remaining_qty: Some(remaining_qty.to_string()),
            last_fill_price: last_fill_price.to_string(),
            avg_fill_price: None,
            last_fill_qty: last_fill_qty.to_string(),
            commission: commission.to_string(),
            commission_asset: "USDT".to_string(),
            trade_time: 1713900000123,
            is_reduce_only: false,
            working_type: "CONTRACT_PRICE".to_string(),
            position_side: None,
        }
    }
}
