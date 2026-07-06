use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio_tungstenite::{connect_async, tungstenite};

// Re-export for convenience
use crate::ws_types::KlineWsClient;
pub use crate::ws_types::{Candle, WsCandleUpdate, WsEvent};

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
        } else if let Some(et) = self.event_type_flat.as_deref() {
            if et == "kline" {
                let symbol = match self.symbol_flat.clone() {
                    Some(s) => s,
                    None => {
                        tracing::warn!("Kline WS message missing symbol — skipping kline");
                        return None;
                    }
                };
                self.kline_flat.map(|kline| BinanceKlineData {
                    event_type: et.to_string(),
                    event_time: 0,
                    symbol,
                    kline,
                })
            } else {
                None
            }
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
    fn to_candle(&self) -> Result<Candle, virs_error::ExchangeError> {
        let symbol = &self.kline.symbol;
        let parse = |field: &str, raw: &str| -> Result<f64, virs_error::ExchangeError> {
            raw.parse::<f64>().map_err(|e| {
                tracing::error!(symbol = %symbol, field = field, raw = %raw, error = %e, "Failed to parse kline OHLCV field — returning NoData instead of 0.0");
                virs_error::ExchangeError::no_data(format!(
                    "kline {field} parse failed for {symbol}: {raw} ({e})"
                ))
            })
        };
        Ok(Candle {
            open_time: self.kline.start_time,
            close_time: self.kline.end_time,
            open: parse("open", &self.kline.open)?,
            high: parse("high", &self.kline.high)?,
            low: parse("low", &self.kline.low)?,
            close: parse("close", &self.kline.close)?,
            volume: parse("volume", &self.kline.volume)?,
            quote_volume: parse("quote_volume", &self.kline.quote_volume)?,
            trades: self.kline.trades,
            closed: self.kline.closed,
        })
    }

    fn ws_symbol(&self) -> &str {
        &self.kline.symbol
    }
}

enum WsCommand {
    Subscribe(String),
    Unsubscribe(String),
}

pub struct KlineWs {
    ws_url: String,
    reconnect_delay_secs: u64,
    max_reconnect_delay_secs: u64,
    ws_ping_interval_secs: u64,
    ws_max_lifetime_secs: u64,
    subscriptions: Arc<RwLock<Vec<String>>>,
    symbol_map: Arc<RwLock<HashMap<String, String>>>,
    running: Arc<AtomicBool>,
    request_id: Arc<AtomicU64>,
    shutdown_tx: Option<mpsc::Sender<()>>,
    command_tx: Option<mpsc::UnboundedSender<WsCommand>>,
}

impl KlineWs {
    pub fn new(
        ws_url: String,
        reconnect_delay_secs: u64,
        max_reconnect_delay_secs: u64,
        ws_ping_interval_secs: u64,
        ws_max_lifetime_secs: u64,
    ) -> Self {
        Self {
            ws_url,
            reconnect_delay_secs,
            max_reconnect_delay_secs,
            ws_ping_interval_secs,
            ws_max_lifetime_secs,
            subscriptions: Arc::new(RwLock::new(Vec::new())),
            symbol_map: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(AtomicBool::new(false)),
            request_id: Arc::new(AtomicU64::new(1)),
            shutdown_tx: None,
            command_tx: None,
        }
    }

    pub fn new_spot(_proxy_url: Option<&str>) -> Self {
        Self::new(
            "wss://stream.binance.com/ws".to_string(),
            1,
            60,
            30,
            23 * 3600,
        )
    }

    pub fn new_perpetual(_proxy_url: Option<&str>) -> Self {
        Self::new(
            "wss://fstream.binance.com/market/ws".to_string(),
            1,
            60,
            30,
            23 * 3600,
        )
    }
}

#[async_trait]
impl KlineWsClient for KlineWs {
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

                match connect_async(&ws_url).await {
                    Ok((ws_stream, _)) => {
                        reconnect_delay = reconnect_delay_secs;

                        if !is_first_connect {
                            if update_tx.send(WsEvent::Reconnected).is_err() {
                                tracing::warn!("[KlineWs] All receivers dropped, stopping");
                                running.store(false, Ordering::Relaxed);
                                break;
                            }
                        }
                        is_first_connect = false;

                        let (mut write, mut read) = ws_stream.split();

                        {
                            // Clone subscriptions and drop the read guard before
                            // async send to avoid holding the lock across .await
                            let subs_vec: Vec<String> = subscriptions.read().await.clone();
                            if !subs_vec.is_empty() {
                                let id = request_id.fetch_add(1, Ordering::Relaxed);
                                let count = subs_vec.len();
                                let msg = serde_json::json!({
                                    "method": "SUBSCRIBE",
                                    "params": subs_vec,
                                    "id": id
                                });
                                if let Ok(text) = serde_json::to_string(&msg) {
                                    match write
                                        .send(tungstenite::Message::Text(text.into()))
                                        .await
                                    {
                                        Ok(()) => {
                                            tracing::info!(
                                                id = id,
                                                count = count,
                                                "[KlineWs] Batch subscription request sent"
                                            );
                                        }
                                        Err(_) => {
                                            tracing::error!(
                                                id = id,
                                                "[KlineWs] Failed to send subscription message"
                                            );
                                            continue;
                                        }
                                    }
                                }
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
                                                            let map = symbol_map.read().await;
                                                            map.get(&raw_sym).cloned().unwrap_or_else(|| raw_sym.clone())
                                                        };
                                                        let candle = match data.to_candle() {
                                                            Ok(c) => c,
                                                            Err(e) => {
                                                                tracing::warn!(
                                                                    symbol = %data.ws_symbol(),
                                                                    error = %e,
                                                                    "Failed to parse kline — skipping this candle update"
                                                                );
                                                                continue;
                                                            }
                                                        };
                                                        if update_tx.send(WsEvent::Candle(WsCandleUpdate {
                                                            symbol: original_symbol,
                                                            candle,
                                                        })).is_err() {
                                                            tracing::warn!("[KlineWs] All receivers dropped, stopping");
                                                            running.store(false, Ordering::Relaxed);
                                                            break;
                                                        }
                                                    } else {
                                                    }
                                                } else {
                                                    // 订阅确认/错误响应（无 kline 数据）
                                                    // 币安成功: {"result": null, "id": N}
                                                    // 币安错误: {"code": 2, "msg": "..."} (合约无 id; 现货含 id)
                                                    if let Ok(resp) =
                                                        serde_json::from_str::<serde_json::Value>(&text)
                                                    {
                                                        if let Some(code) = resp.get("code") {
                                                            tracing::error!(
                                                                id = ?resp.get("id"),
                                                                code = ?code,
                                                                msg = ?resp.get("msg"),
                                                                "[KlineWs] Subscription rejected by Binance"
                                                            );
                                                        } else if resp.get("result").is_some() {
                                                            tracing::info!(
                                                                id = ?resp.get("id"),
                                                                "[KlineWs] Subscription confirmed by Binance"
                                                            );
                                                        }
                                                    }
                                                }
                                            } else {
                                                tracing::warn!("[KlineWs] Failed to parse WS message: {}", &text[..text.len().min(200)]);
                                            }
                                        }
                                        Some(Ok(tungstenite::Message::Ping(data))) => {
                                            let _ = write.send(tungstenite::Message::Pong(data)).await;
                                        }
                                        Some(Ok(tungstenite::Message::Close(_))) => {
                                            tracing::warn!("[KlineWs] Server closed connection");
                                            break;
                                        }
                                        Some(Err(e)) => {
                                            tracing::error!("[KlineWs] Read error: {}", e);
                                            break;
                                        }
                                        None => {
                                            tracing::warn!("[KlineWs] Stream ended");
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                                _ = ping_tick.tick() => {
                                    let ping = tungstenite::Message::Ping(vec![].into());
                                    if write.send(ping).await.is_err() {
                                        tracing::warn!("[KlineWs] Ping failed, reconnecting...");
                                        break;
                                    }
                                }
                                cmd = command_rx.recv() => {
                                    match cmd {
                                        Some(WsCommand::Subscribe(stream_name)) => {
                                            let id = request_id.fetch_add(1, Ordering::Relaxed);
                                            let msg = serde_json::json!({
                                                "method": "SUBSCRIBE",
                                                "params": [stream_name.clone()],
                                                "id": id
                                            });
                                            if let Ok(text) = serde_json::to_string(&msg) {
                                                match write.send(tungstenite::Message::Text(text.into())).await {
                                                    Ok(()) => {
                                                        tracing::info!(
                                                            id = id,
                                                            stream = %stream_name,
                                                            "[KlineWs] Dynamic subscribe request sent"
                                                        );
                                                    }
                                                    Err(_) => {
                                                        tracing::error!(
                                                            id = id,
                                                            stream = %stream_name,
                                                            "[KlineWs] Failed to send dynamic subscribe"
                                                        );
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                        Some(WsCommand::Unsubscribe(stream_name)) => {
                                            let id = request_id.fetch_add(1, Ordering::Relaxed);
                                            let msg = serde_json::json!({
                                                "method": "UNSUBSCRIBE",
                                                "params": [stream_name.clone()],
                                                "id": id
                                            });
                                            if let Ok(text) = serde_json::to_string(&msg) {
                                                match write.send(tungstenite::Message::Text(text.into())).await {
                                                    Ok(()) => {
                                                        tracing::info!(
                                                            id = id,
                                                            stream = %stream_name,
                                                            "[KlineWs] Dynamic unsubscribe request sent"
                                                        );
                                                    }
                                                    Err(_) => {
                                                        tracing::error!(
                                                            id = id,
                                                            stream = %stream_name,
                                                            "[KlineWs] Failed to send dynamic unsubscribe"
                                                        );
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                        None => {
                                            break;
                                        }
                                    }
                                }
                                _ = shutdown_rx.recv() => {
                                    let _ = write.send(tungstenite::Message::Close(None)).await;
                                    running.store(false, Ordering::Relaxed);
                                    return;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("[KlineWs] Connection failed: {}", e);
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
            let mut map = self.symbol_map.write().await;
            map.insert(ws_sym, symbol.to_string());
        }

        let mut subs = self.subscriptions.write().await;
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
            let mut map = self.symbol_map.write().await;
            map.remove(&ws_sym);
        }

        let mut subs = self.subscriptions.write().await;
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

        let candle = data.to_candle().expect("valid candle");
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
        let candle_closed = data_closed.to_candle().expect("valid closed candle");
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

        let result = data.to_candle();
        assert!(result.is_err(), "invalid OHLCV fields must return Err, not 0.0");
        let err = result.unwrap_err();
        assert!(
            matches!(err, virs_error::ExchangeError::NoData(_)),
            "expected NoData error, got {err:?}"
        );
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
        let ws = KlineWs::new_spot(None);
        assert_eq!(ws.ws_url, "wss://stream.binance.com/ws");
        assert!(!ws.is_running());
    }

    #[test]
    fn test_new_perpetual() {
        let ws = KlineWs::new_perpetual(None);
        assert_eq!(ws.ws_url, "wss://fstream.binance.com/market/ws");
        assert!(!ws.is_running());
    }

    #[tokio::test]
    async fn test_subscribe_without_start() {
        let ws = KlineWs::new_spot(None);
        assert!(!ws.is_running());

        // 不调用 start()，直接调用 subscribe
        ws.subscribe("BTCUSDT").await;

        // 验证 subscriptions 包含正确的 stream name
        let subs = ws.subscriptions.read().await;
        assert!(subs.contains(&"btcusdt@kline_1m".to_string()));

        // 验证 symbol_map 包含映射
        let map = ws.symbol_map.read().await;
        assert_eq!(map.get("btcusdt").unwrap(), "BTCUSDT");

        // 客户端仍然没有运行
        assert!(!ws.is_running());
    }
}
