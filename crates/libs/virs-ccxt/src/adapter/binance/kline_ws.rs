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

pub(crate) fn binance_ws_symbol(symbol: &str) -> String {
    symbol.replace('/', "").to_lowercase()
}

/// Binance WS 推送两种格式：
/// 1. 单流格式: {"e":"kline", "s":"BTCUSDT", "k":{...}}
/// 2. 组合流格式: {"stream":"btcusdt@kline_1m", "data":{"e":"kline","k":{...}}}
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BinanceKlineMessage {
    #[allow(dead_code)]
    pub(crate) stream: Option<String>,
    /// 组合流格式: data 字段包含完整的 kline 事件
    pub(crate) data: Option<BinanceKlineData>,
    /// 单流格式: 顶层直接包含 kline 事件字段
    #[serde(rename = "e")]
    pub(crate) event_type_flat: Option<String>,
    /// T8 FAIL fix: 单流格式的事件时间（E 字段），此前缺失导致 event_time 恒为 0
    #[serde(rename = "E")]
    pub(crate) event_time_flat: Option<i64>,
    #[serde(rename = "s")]
    pub(crate) symbol_flat: Option<String>,
    #[serde(rename = "k")]
    pub(crate) kline_flat: Option<BinanceKlineInner>,
}

impl BinanceKlineMessage {
    /// 提取 kline 数据，兼容单流和组合流两种格式
    pub(crate) fn into_kline_data(self) -> Option<BinanceKlineData> {
        if let Some(data) = self.data {
            Some(data)
        } else if let Some(et) = self.event_type_flat.as_deref() {
            if et == "kline" {
                if self.symbol_flat.is_none() {
                    tracing::warn!("Kline WS message missing symbol — skipping kline");
                    return None;
                }
                // T8 FAIL fix: 使用实际解析到的 E 字段，而非硬编码 0
                let event_time = self.event_time_flat.unwrap_or(0);
                self.kline_flat.map(|kline| BinanceKlineData {
                    event_type: et.to_string(),
                    event_time,
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

/// WS 消息延迟告警阈值（毫秒）
pub(crate) const KLINE_WS_DELAY_THRESHOLD_MS: i64 = 5_000;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BinanceKlineData {
    #[serde(rename = "e")]
    pub(crate) event_type: String,
    #[serde(rename = "E")]
    pub(crate) event_time: i64,
    #[serde(rename = "k")]
    pub(crate) kline: BinanceKlineInner,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BinanceKlineInner {
    #[serde(rename = "t")]
    pub(crate) start_time: i64,
    #[serde(rename = "T")]
    pub(crate) end_time: i64,
    #[serde(rename = "s")]
    pub(crate) symbol: String,
    #[allow(dead_code)]
    #[serde(rename = "i")]
    pub(crate) interval: String,
    #[serde(rename = "o")]
    pub(crate) open: String,
    #[serde(rename = "h")]
    pub(crate) high: String,
    #[serde(rename = "l")]
    pub(crate) low: String,
    #[serde(rename = "c")]
    pub(crate) close: String,
    #[serde(rename = "v")]
    pub(crate) volume: String,
    #[serde(rename = "n")]
    pub(crate) trades: i64,
    #[serde(rename = "x")]
    pub(crate) closed: bool,
    #[serde(rename = "q")]
    pub(crate) quote_volume: String,
}

impl BinanceKlineData {
    pub(crate) fn to_candle(&self) -> Result<Candle, virs_error::ExchangeError> {
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

    pub(crate) fn ws_symbol(&self) -> &str {
        &self.kline.symbol
    }
}

enum WsCommand {
    Subscribe(String),
    Unsubscribe(String),
}

pub struct KlineWs {
    pub(crate) ws_url: String,
    reconnect_delay_secs: u64,
    max_reconnect_delay_secs: u64,
    ws_ping_interval_secs: u64,
    ws_max_lifetime_secs: u64,
    pub(crate) subscriptions: Arc<RwLock<Vec<String>>>,
    pub(crate) symbol_map: Arc<RwLock<HashMap<String, String>>>,
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

    pub fn new_perpetual(
        _proxy_url: Option<&str>,
        reconnect_delay_secs: u64,
        max_reconnect_delay_secs: u64,
        ws_ping_interval_secs: u64,
        ws_max_lifetime_secs: u64,
    ) -> Self {
        Self::new(
            "wss://fstream.binance.com/market/ws".to_string(),
            reconnect_delay_secs,
            max_reconnect_delay_secs,
            ws_ping_interval_secs,
            ws_max_lifetime_secs,
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
                                                    // T8: Detect WS message delay using event_time
                                                    if data.event_time > 0 {
                                                        let local_now = chrono::Utc::now().timestamp_millis();
                                                        let delay_ms = local_now - data.event_time;
                                                        if delay_ms > KLINE_WS_DELAY_THRESHOLD_MS {
                                                            tracing::warn!(
                                                                delay_ms = delay_ms,
                                                                event_time = data.event_time,
                                                                local_time = local_now,
                                                                symbol = %data.kline.symbol,
                                                                "[KlineWs] Message delay exceeds threshold"
                                                            );
                                                        }
                                                    }
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
