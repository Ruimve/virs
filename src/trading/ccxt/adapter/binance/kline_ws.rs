use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_tungstenite::{connect_async, tungstenite};

// Re-export for convenience
pub use crate::engine::kline::types::{Candle, WsCandleUpdate, WsEvent};
use crate::engine::kline::types::KlineWsClient;

fn binance_ws_symbol(symbol: &str) -> String {
    symbol.replace('/', "").to_lowercase()
}

/// Binance WS 推送两种格式：
/// 1. 单流格式: {"e":"kline", "s":"BTCUSDT", "k":{...}}
/// 2. 组合流格式: {"stream":"btcusdt@kline_1m", "data":{"e":"kline","k":{...}}}
#[derive(Debug, Clone, Deserialize)]
struct BinanceKlineMessage {
    #[allow(dead_code)]
    stream: Option<String>,
    /// 组合流格式: data 字段包含完整的 kline 事件
    data: Option<BinanceKlineData>,
    /// 单流格式: 顶层直接包含 kline 事件字段
    #[serde(rename = "e")]
    event_type_flat: Option<String>,
    #[serde(rename = "s")]
    symbol_flat: Option<String>,
    #[serde(rename = "k")]
    kline_flat: Option<BinanceKlineInner>,
}

impl BinanceKlineMessage {
    /// 提取 kline 数据，兼容单流和组合流两种格式
    fn into_kline_data(self) -> Option<BinanceKlineData> {
        if let Some(data) = self.data {
            Some(data)
        } else if self.event_type_flat.as_deref() == Some("kline") {
            self.kline_flat.map(|kline| BinanceKlineData {
                event_type: self.event_type_flat.unwrap(),
                event_time: 0,
                symbol: self.symbol_flat.unwrap_or_default(),
                kline,
            })
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct BinanceKlineData {
    #[serde(rename = "e")]
    event_type: String,
    #[allow(dead_code)]
    #[serde(rename = "E")]
    event_time: i64,
    #[allow(dead_code)]
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "k")]
    kline: BinanceKlineInner,
}

#[derive(Debug, Clone, Deserialize)]
struct BinanceKlineInner {
    #[serde(rename = "t")]
    start_time: i64,
    #[serde(rename = "T")]
    end_time: i64,
    #[serde(rename = "s")]
    symbol: String,
    #[allow(dead_code)]
    #[serde(rename = "i")]
    interval: String,
    #[serde(rename = "o")]
    open: String,
    #[serde(rename = "h")]
    high: String,
    #[serde(rename = "l")]
    low: String,
    #[serde(rename = "c")]
    close: String,
    #[serde(rename = "v")]
    volume: String,
    #[serde(rename = "n")]
    trades: i64,
    #[serde(rename = "x")]
    closed: bool,
    #[serde(rename = "q")]
    quote_volume: String,
}

impl BinanceKlineData {
    fn to_candle(&self) -> Candle {
        Candle {
            open_time: self.kline.start_time,
            close_time: self.kline.end_time,
            open: self.kline.open.parse().unwrap_or(0.0),
            high: self.kline.high.parse().unwrap_or(0.0),
            low: self.kline.low.parse().unwrap_or(0.0),
            close: self.kline.close.parse().unwrap_or(0.0),
            volume: self.kline.volume.parse().unwrap_or(0.0),
            quote_volume: self.kline.quote_volume.parse().unwrap_or(0.0),
            trades: self.kline.trades,
            closed: self.kline.closed,
        }
    }

    fn ws_symbol(&self) -> &str {
        &self.kline.symbol
    }
}

enum WsCommand {
    Subscribe(String),
    Unsubscribe(String),
}

pub struct BinanceKlineWs {
    ws_url: String,
    reconnect_delay_secs: u64,
    max_reconnect_delay_secs: u64,
    ws_ping_interval_secs: u64,
    ws_max_lifetime_secs: u64,
    subscriptions: Arc<Mutex<Vec<String>>>,
    symbol_map: Arc<Mutex<HashMap<String, String>>>,
    running: Arc<AtomicBool>,
    request_id: Arc<AtomicU64>,
    shutdown_tx: Option<mpsc::Sender<()>>,
    command_tx: Option<mpsc::UnboundedSender<WsCommand>>,
}

impl BinanceKlineWs {
    pub fn new(ws_url: String, reconnect_delay_secs: u64, max_reconnect_delay_secs: u64, ws_ping_interval_secs: u64, ws_max_lifetime_secs: u64) -> Self {
        Self {
            ws_url,
            reconnect_delay_secs,
            max_reconnect_delay_secs,
            ws_ping_interval_secs,
            ws_max_lifetime_secs,
            subscriptions: Arc::new(Mutex::new(Vec::new())),
            symbol_map: Arc::new(Mutex::new(HashMap::new())),
            running: Arc::new(AtomicBool::new(false)),
            request_id: Arc::new(AtomicU64::new(1)),
            shutdown_tx: None,
            command_tx: None,
        }
    }

    pub fn new_spot(_proxy_url: Option<&str>) -> Self {
        Self::new(
            "wss://stream.binance.com/ws".to_string(),
            1, 60, 30, 23 * 3600,
        )
    }

    pub fn new_perpetual(_proxy_url: Option<&str>) -> Self {
        Self::new(
            "wss://fstream.binance.com/market/ws".to_string(),
            1, 60, 30, 23 * 3600,
        )
    }

    pub async fn subscription_count(&self) -> usize {
        self.subscriptions.lock().await.len()
    }
}

#[async_trait]
impl KlineWsClient for BinanceKlineWs {
    async fn start(&mut self, update_tx: broadcast::Sender<WsEvent>) {
        if self.running.load(Ordering::Relaxed) {
            return;
        }
        self.running.store(true, Ordering::Relaxed);

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx);

        let (command_tx, mut command_rx) = mpsc::unbounded_channel::<WsCommand>();
        self.command_tx = Some(command_tx);

        let ws_url = self.ws_url.clone();
        let subscriptions = self.subscriptions.clone();
        let symbol_map = self.symbol_map.clone();
        let running = self.running.clone();
        let request_id = self.request_id.clone();
        let reconnect_delay_secs = self.reconnect_delay_secs;
        let max_reconnect_delay_secs = self.max_reconnect_delay_secs;
        let ws_ping_interval_secs = self.ws_ping_interval_secs;
        let ws_max_lifetime_secs = self.ws_max_lifetime_secs;

        tokio::spawn(async move {
            let mut reconnect_delay = reconnect_delay_secs;
            let mut is_first_connect = true;

            while running.load(Ordering::Relaxed) {
                let connect_start = tokio::time::Instant::now();

                tracing::debug!("[BinanceKlineWs] Connecting to {}...", ws_url);

                match connect_async(&ws_url).await {
                    Ok((ws_stream, _)) => {
                        tracing::info!("[BinanceKlineWs] Connected to {} successfully", ws_url);
                        reconnect_delay = reconnect_delay_secs;

                        if !is_first_connect {
                            let _ = update_tx.send(WsEvent::Reconnected);
                        }
                        is_first_connect = false;

                        let (mut write, mut read) = ws_stream.split();

                        {
                            let subs = subscriptions.lock().await;
                            if !subs.is_empty() {
                                let id = request_id.fetch_add(1, Ordering::Relaxed);
                                let subs_vec: Vec<&String> = subs.iter().collect();
                                tracing::info!("[BinanceKlineWs] Connected to {} (subscribing {} streams)", ws_url, subs.len());
                                let msg = serde_json::json!({
                                    "method": "SUBSCRIBE",
                                    "params": subs_vec,
                                    "id": id
                                });
                                if let Ok(text) = serde_json::to_string(&msg) {
                                    if write.send(tungstenite::Message::Text(text.into())).await.is_err() {
                                        tracing::error!("[BinanceKlineWs] Failed to send subscription message");
                                        continue;
                                    }
                                }
                            } else {
                                tracing::warn!("[BinanceKlineWs] No streams to subscribe (subscriptions empty)");
                            }
                        }

                        let ping_interval = Duration::from_secs(ws_ping_interval_secs);
                        let mut ping_tick = tokio::time::interval(ping_interval);
                        let max_lifetime = Duration::from_secs(ws_max_lifetime_secs);

                        loop {
                            if !running.load(Ordering::Relaxed) {
                                break;
                            }

                            if connect_start.elapsed() > max_lifetime {
                                tracing::info!("[BinanceKlineWs] Max lifetime reached, reconnecting...");
                                break;
                            }

                            tokio::select! {
                                msg = read.next() => {
                                    match msg {
                                        Some(Ok(tungstenite::Message::Text(text))) => {
                                            if let Ok(bmsg) = serde_json::from_str::<BinanceKlineMessage>(&text) {
                                                if let Some(data) = bmsg.into_kline_data() {
                                                    if data.event_type == "kline" {
                                                        let raw_sym = data.ws_symbol().to_lowercase();
                                                        let original_symbol = {
                                                            let map = symbol_map.lock().await;
                                                            map.get(&raw_sym).cloned().unwrap_or_else(|| raw_sym.clone())
                                                        };
                                                        let candle = data.to_candle();
                                                        tracing::debug!(
                                                            "[BinanceKlineWs] 1m kline: {} open_time={} close={:.2} closed={}",
                                                            original_symbol, candle.open_time, candle.close, candle.closed
                                                        );
                                                        let _ = update_tx.send(WsEvent::Candle(WsCandleUpdate {
                                                            symbol: original_symbol,
                                                            candle,
                                                        }));
                                                    } else {
                                                        tracing::debug!("[BinanceKlineWs] Received non-kline event: {}", data.event_type);
                                                    }
                                                } else {
                                                    // 订阅确认/错误响应（无 kline 数据）
                                                    tracing::debug!("[BinanceKlineWs] Received WS message (no kline data): {}", &text[..text.len().min(200)]);
                                                }
                                            } else {
                                                tracing::warn!("[BinanceKlineWs] Failed to parse WS message: {}", &text[..text.len().min(200)]);
                                            }
                                        }
                                        Some(Ok(tungstenite::Message::Ping(data))) => {
                                            let _ = write.send(tungstenite::Message::Pong(data)).await;
                                        }
                                        Some(Ok(tungstenite::Message::Close(_))) => {
                                            tracing::warn!("[BinanceKlineWs] Server closed connection");
                                            break;
                                        }
                                        Some(Err(e)) => {
                                            tracing::error!("[BinanceKlineWs] Read error: {}", e);
                                            break;
                                        }
                                        None => {
                                            tracing::warn!("[BinanceKlineWs] Stream ended");
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                                _ = ping_tick.tick() => {
                                    let ping = tungstenite::Message::Ping(vec![].into());
                                    if write.send(ping).await.is_err() {
                                        tracing::warn!("[BinanceKlineWs] Ping failed, reconnecting...");
                                        break;
                                    }
                                }
                                cmd = command_rx.recv() => {
                                    match cmd {
                                        Some(WsCommand::Subscribe(stream_name)) => {
                                            let id = request_id.fetch_add(1, Ordering::Relaxed);
                                            let msg = serde_json::json!({
                                                "method": "SUBSCRIBE",
                                                "params": [stream_name],
                                                "id": id
                                            });
                                            if let Ok(text) = serde_json::to_string(&msg) {
                                                if write.send(tungstenite::Message::Text(text.into())).await.is_err() {
                                                    tracing::warn!("[BinanceKlineWs] Failed to send dynamic subscribe");
                                                    break;
                                                }
                                                tracing::debug!("[BinanceKlineWs] Dynamically subscribed to stream");
                                            }
                                        }
                                        Some(WsCommand::Unsubscribe(stream_name)) => {
                                            let id = request_id.fetch_add(1, Ordering::Relaxed);
                                            let msg = serde_json::json!({
                                                "method": "UNSUBSCRIBE",
                                                "params": [stream_name],
                                                "id": id
                                            });
                                            if let Ok(text) = serde_json::to_string(&msg) {
                                                if write.send(tungstenite::Message::Text(text.into())).await.is_err() {
                                                    tracing::warn!("[BinanceKlineWs] Failed to send dynamic unsubscribe");
                                                    break;
                                                }
                                                tracing::debug!("[BinanceKlineWs] Dynamically unsubscribed from stream");
                                            }
                                        }
                                        None => {
                                            tracing::debug!("[BinanceKlineWs] Command channel closed");
                                            break;
                                        }
                                    }
                                }
                                _ = shutdown_rx.recv() => {
                                    tracing::debug!("[BinanceKlineWs] Shutdown requested");
                                    let _ = write.send(tungstenite::Message::Close(None)).await;
                                    running.store(false, Ordering::Relaxed);
                                    return;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("[BinanceKlineWs] Connection failed: {}", e);
                    }
                }

                if !running.load(Ordering::Relaxed) {
                    break;
                }

                tracing::debug!("[BinanceKlineWs] Reconnecting in {}s...", reconnect_delay);
                tokio::time::sleep(Duration::from_secs(reconnect_delay)).await;
                reconnect_delay = (reconnect_delay * 2).min(max_reconnect_delay_secs);
            }

            running.store(false, Ordering::Relaxed);
            tracing::debug!("[BinanceKlineWs] Worker exited");
        });
    }

    async fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }
    }

    async fn subscribe(&self, symbol: &str) {
        let stream_name = format!("{}@kline_1m", binance_ws_symbol(symbol));
        let ws_sym = binance_ws_symbol(symbol);

        {
            let mut map = self.symbol_map.lock().await;
            map.insert(ws_sym, symbol.to_string());
        }

        let mut subs = self.subscriptions.lock().await;
        if !subs.contains(&stream_name) {
            subs.push(stream_name.clone());
            drop(subs);
            if let Some(tx) = &self.command_tx {
                let _ = tx.send(WsCommand::Subscribe(stream_name));
            }
        }
    }

    async fn unsubscribe(&self, symbol: &str) {
        let stream_name = format!("{}@kline_1m", binance_ws_symbol(symbol));
        let ws_sym = binance_ws_symbol(symbol);

        {
            let mut map = self.symbol_map.lock().await;
            map.remove(&ws_sym);
        }

        let mut subs = self.subscriptions.lock().await;
        let existed = subs.iter().any(|s| s == &stream_name);
        subs.retain(|s| s != &stream_name);
        drop(subs);
        if existed {
            if let Some(tx) = &self.command_tx {
                let _ = tx.send(WsCommand::Unsubscribe(stream_name));
            }
        }
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::kline::types::Candle;

    // ========== 消息解析（5个） ==========

    #[test]
    fn test_parse_binance_kline_message() {
        let json = r#"{
            "stream": "btcusdt@kline_1m",
            "data": {
                "e": "kline",
                "E": 1713900000,
                "s": "BTCUSDT",
                "k": {
                    "t": 1713900000000,
                    "T": 1713900059999,
                    "s": "BTCUSDT",
                    "i": "1m",
                    "o": "65000.00",
                    "h": "65100.00",
                    "l": "64900.00",
                    "c": "65050.00",
                    "v": "100.5",
                    "n": 500,
                    "x": false,
                    "q": "6532500.00"
                }
            }
        }"#;

        let msg: BinanceKlineMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.stream.as_deref(), Some("btcusdt@kline_1m"));
        assert!(msg.data.is_some());

        let data = msg.data.unwrap();
        assert_eq!(data.event_type, "kline");
        assert_eq!(data.kline.start_time, 1713900000000);
        assert_eq!(data.kline.end_time, 1713900059999);
        assert_eq!(data.kline.open, "65000.00");
        assert_eq!(data.kline.high, "65100.00");
        assert_eq!(data.kline.low, "64900.00");
        assert_eq!(data.kline.close, "65050.00");
        assert_eq!(data.kline.volume, "100.5");
        assert_eq!(data.kline.trades, 500);
        assert!(!data.kline.closed);
        assert_eq!(data.kline.quote_volume, "6532500.00");

        // Closed kline assertion (merged from test_parse_binance_kline_closed)
        let json_closed = r#"{
            "stream": "btcusdt@kline_1m",
            "data": {
                "e": "kline",
                "E": 1713900000,
                "s": "BTCUSDT",
                "k": {
                    "t": 1713900000000,
                    "T": 1713900059999,
                    "s": "BTCUSDT",
                    "i": "1m",
                    "o": "65000.00",
                    "h": "65100.00",
                    "l": "64900.00",
                    "c": "65050.00",
                    "v": "100.5",
                    "n": 500,
                    "x": true,
                    "q": "6532500.00"
                }
            }
        }"#;
        let msg_closed: BinanceKlineMessage = serde_json::from_str(json_closed).unwrap();
        let data_closed = msg_closed.data.unwrap();
        assert!(data_closed.kline.closed);

        // 单流格式（无 stream/data 包装）
        let json_flat = r#"{
            "e": "kline",
            "E": 1713900000,
            "s": "BTCUSDT",
            "k": {
                "t": 1713900000000,
                "T": 1713900059999,
                "s": "BTCUSDT",
                "i": "1m",
                "o": "65000.00",
                "h": "65100.00",
                "l": "64900.00",
                "c": "65050.00",
                "v": "100.5",
                "n": 500,
                "x": true,
                "q": "6532500.00"
            }
        }"#;
        let msg_flat: BinanceKlineMessage = serde_json::from_str(json_flat).unwrap();
        assert!(msg_flat.stream.is_none());
        assert!(msg_flat.data.is_none());
        assert_eq!(msg_flat.event_type_flat.as_deref(), Some("kline"));
        let data_flat = msg_flat.into_kline_data().unwrap();
        assert_eq!(data_flat.event_type, "kline");
        assert_eq!(data_flat.kline.start_time, 1713900000000);
        assert!(data_flat.kline.closed);
    }


    #[test]
    fn test_parse_binance_kline_message_without_stream() {
        let json = r#"{
            "data": {
                "e": "kline",
                "E": 1713900000,
                "s": "BTCUSDT",
                "k": {
                    "t": 1713900000000,
                    "T": 1713900059999,
                    "s": "BTCUSDT",
                    "i": "1m",
                    "o": "65000.00",
                    "h": "65100.00",
                    "l": "64900.00",
                    "c": "65050.00",
                    "v": "100.5",
                    "n": 500,
                    "x": false,
                    "q": "6532500.00"
                }
            }
        }"#;

        let msg: BinanceKlineMessage = serde_json::from_str(json).unwrap();
        assert!(msg.stream.is_none());
        assert!(msg.data.is_some());
    }

    #[test]
    fn test_parse_invalid_json() {
        let result: Result<BinanceKlineMessage, _> = serde_json::from_str("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_non_kline_event() {
        let json = r#"{
            "stream": "btcusdt@trade",
            "data": {
                "e": "trade",
                "E": 1713900000,
                "s": "BTCUSDT",
                "t": 12345,
                "p": "65000.00",
                "q": "1.5",
                "b": 100,
                "a": 200,
                "T": 1713900000123,
                "m": true,
                "M": true
            }
        }"#;

        // trade 事件没有 "k" 字段，反序列化应该失败
        let result: Result<BinanceKlineMessage, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    // ========== Candle 转换（4个） ==========

    #[test]
    fn test_to_candle_basic() {
        let data = BinanceKlineData {
            event_type: "kline".to_string(),
            event_time: 1713900000,
            symbol: "BTCUSDT".to_string(),
            kline: BinanceKlineInner {
                start_time: 1713900000000,
                end_time: 1713900059999,
                symbol: "BTCUSDT".to_string(),
                interval: "1m".to_string(),
                open: "65000.00".to_string(),
                high: "65100.00".to_string(),
                low: "64900.00".to_string(),
                close: "65050.00".to_string(),
                volume: "100.5".to_string(),
                trades: 500,
                closed: false,
                quote_volume: "6532500.00".to_string(),
            },
        };

        let candle = data.to_candle();
        assert_eq!(candle.open_time, 1713900000000);
        assert_eq!(candle.close_time, 1713900059999);
        assert!((candle.open - 65000.0).abs() < f64::EPSILON);
        assert!((candle.high - 65100.0).abs() < f64::EPSILON);
        assert!((candle.low - 64900.0).abs() < f64::EPSILON);
        assert!((candle.close - 65050.0).abs() < f64::EPSILON);
        assert!((candle.volume - 100.5).abs() < f64::EPSILON);
        assert!((candle.quote_volume - 6532500.0).abs() < f64::EPSILON);
        assert_eq!(candle.trades, 500);
        assert!(!candle.closed);

        // Closed candle assertion (merged from test_to_candle_closed)
        let data_closed = BinanceKlineData {
            event_type: "kline".to_string(),
            event_time: 1713900000,
            symbol: "BTCUSDT".to_string(),
            kline: BinanceKlineInner {
                start_time: 1713900000000,
                end_time: 1713900059999,
                symbol: "BTCUSDT".to_string(),
                interval: "1m".to_string(),
                open: "65000.00".to_string(),
                high: "65100.00".to_string(),
                low: "64900.00".to_string(),
                close: "65050.00".to_string(),
                volume: "100.5".to_string(),
                trades: 500,
                closed: true,
                quote_volume: "6532500.00".to_string(),
            },
        };
        let candle_closed = data_closed.to_candle();
        assert!(candle_closed.closed);
    }


    #[test]
    fn test_to_candle_invalid_numbers() {
        let data = BinanceKlineData {
            event_type: "kline".to_string(),
            event_time: 1713900000,
            symbol: "BTCUSDT".to_string(),
            kline: BinanceKlineInner {
                start_time: 1713900000000,
                end_time: 1713900059999,
                symbol: "BTCUSDT".to_string(),
                interval: "1m".to_string(),
                open: "not_a_number".to_string(),
                high: "abc".to_string(),
                low: "64900.00".to_string(),
                close: "65050.00".to_string(),
                volume: "100.5".to_string(),
                trades: 500,
                closed: false,
                quote_volume: "6532500.00".to_string(),
            },
        };

        let candle = data.to_candle();
        assert!((candle.open - 0.0).abs() < f64::EPSILON);
        assert!((candle.high - 0.0).abs() < f64::EPSILON);
        // 正常字段不受影响
        assert!((candle.low - 64900.0).abs() < f64::EPSILON);
        assert!((candle.close - 65050.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_ws_symbol() {
        let data = BinanceKlineData {
            event_type: "kline".to_string(),
            event_time: 1713900000,
            symbol: "BTCUSDT".to_string(),
            kline: BinanceKlineInner {
                start_time: 1713900000000,
                end_time: 1713900059999,
                symbol: "BTCUSDT".to_string(),
                interval: "1m".to_string(),
                open: "65000.00".to_string(),
                high: "65100.00".to_string(),
                low: "64900.00".to_string(),
                close: "65050.00".to_string(),
                volume: "100.5".to_string(),
                trades: 500,
                closed: false,
                quote_volume: "6532500.00".to_string(),
            },
        };

        assert_eq!(data.ws_symbol(), "BTCUSDT");
    }

    // ========== Symbol 转换（3个） ==========

    #[test]
    fn test_binance_ws_symbol_basic() {
        assert_eq!(binance_ws_symbol("BTCUSDT"), "btcusdt");
        // With slash (merged from test_binance_ws_symbol_with_slash)
        assert_eq!(binance_ws_symbol("BTC/USDT"), "btcusdt");
        // Already lowercase (merged from test_binance_ws_symbol_lowercase)
        assert_eq!(binance_ws_symbol("btcusdt"), "btcusdt");
    }


    // ========== 构造函数和状态（3个） ==========

    #[test]
    fn test_new_spot() {
        let ws = BinanceKlineWs::new_spot(None);
        assert_eq!(ws.ws_url, "wss://stream.binance.com/ws");
        assert!(!ws.is_running());
    }

    #[test]
    fn test_new_perpetual() {
        let ws = BinanceKlineWs::new_perpetual(None);
        assert_eq!(ws.ws_url, "wss://fstream.binance.com/market/ws");
        assert!(!ws.is_running());
    }

    #[tokio::test]
    async fn test_subscribe_without_start() {
        let ws = BinanceKlineWs::new_spot(None);
        assert!(!ws.is_running());

        // 不调用 start()，直接调用 subscribe
        ws.subscribe("BTCUSDT").await;

        // 验证 subscriptions 包含正确的 stream name
        let subs = ws.subscriptions.lock().await;
        assert!(subs.contains(&"btcusdt@kline_1m".to_string()));

        // 验证 symbol_map 包含映射
        let map = ws.symbol_map.lock().await;
        assert_eq!(map.get("btcusdt").unwrap(), "BTCUSDT");

        // 客户端仍然没有运行
        assert!(!ws.is_running());
    }
}
