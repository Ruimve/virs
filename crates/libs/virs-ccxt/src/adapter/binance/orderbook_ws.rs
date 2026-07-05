//! Binance order book WebSocket client.
//!
//! Subscribes to `<symbol>@depth20@500ms` partial book depth streams
//! (top 20 bid/ask levels, pushed every 500ms).
//!
//! Mirrors `KlineWs` architecture: reconnect with backoff,
//! dynamic subscribe/unsubscribe, symbol map for unified format.
//!
//! Stream formats:
//! - Spot partial book depth:    { "lastUpdateId": 160, "bids": [[p,a]], "asks": [[p,a]] }
//! - Perpetual partial book depth: { "e":"depthUpdate", "E":..., "T":..., "s":"BTCUSDT",
//!   "U":..., "u":..., "pu":..., "b":[[p,a]], "a":[[p,a]] }
//! - Combined stream wrapper:    { "stream": "<name>", "data": <payload> }

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio_tungstenite::{connect_async, tungstenite};

use crate::ws_types::{OrderBookLevel, OrderBookWsClient, WsOrderBookEvent, WsOrderBookUpdate};

fn binance_ws_symbol(symbol: &str) -> String {
    symbol.replace('/', "").to_lowercase()
}

/// Binance WS 推送两种格式（与 kline 一致）：
/// 1. 单流格式: 顶层直接是 payload
/// 2. 组合流格式: {"stream":"<name>", "data": <payload>}
///
/// 此外，spot 和 perpetual 的 payload 字段名不同：
/// - Spot:      bids / asks / lastUpdateId
/// - Perpetual: b    / a    / e / E / T / s / U / u / pu
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BinanceDepthMessage {
    #[allow(dead_code)]
    stream: Option<String>,
    /// 组合流格式: data 字段包含完整 payload
    data: Option<serde_json::Value>,
    /// 单流 spot 格式: 顶层 bids/asks
    bids: Option<Vec<[String; 2]>>,
    asks: Option<Vec<[String; 2]>>,
    #[serde(rename = "lastUpdateId")]
    last_update_id: Option<i64>,
    /// 单流 perpetual 格式: 顶层 b/a
    #[serde(rename = "e")]
    #[allow(dead_code)]
    event_type_flat: Option<String>,
    #[serde(rename = "E")]
    event_time_flat: Option<i64>,
    #[serde(rename = "T")]
    #[allow(dead_code)]
    transaction_time_flat: Option<i64>,
    #[serde(rename = "s")]
    symbol_flat: Option<String>,
    #[serde(rename = "b")]
    bids_perp_flat: Option<Vec<[String; 2]>>,
    #[serde(rename = "a")]
    asks_perp_flat: Option<Vec<[String; 2]>>,
}

impl BinanceDepthMessage {
    /// 提取订单簿数据，兼容单流/组合流 + spot/perpetual
    /// Returns: (bids, asks, stream_name, symbol_from_payload, timestamp_ms)
    pub(crate) fn into_depth(
        self,
    ) -> Option<(
        Vec<[String; 2]>,
        Vec<[String; 2]>,
        Option<String>,
        Option<String>,
        i64,
    )> {
        let stream = self.stream.clone();

        // 组合流：解析 data
        if let Some(data) = self.data {
            if let Some((bids, asks, sym, ts)) = parse_payload(&data) {
                return Some((bids, asks, stream, sym, ts));
            }
            return None;
        }

        // 单流 spot: bids/asks at top level
        if let (Some(bids), Some(asks)) = (self.bids.clone(), self.asks.clone()) {
            let ts = self.last_update_id.unwrap_or_else(|| {
                tracing::warn!("orderbook_ws: last_update_id is None — using 0 as fallback timestamp");
                0
            });
            return Some((bids, asks, stream, None, ts));
        }

        // 单流 perpetual: b/a at top level
        if let (Some(bids), Some(asks)) = (self.bids_perp_flat, self.asks_perp_flat) {
            let ts = self.event_time_flat.unwrap_or_else(|| {
                tracing::warn!("orderbook_ws: event_time_flat is None — using 0 as fallback timestamp");
                0
            });
            return Some((bids, asks, stream, self.symbol_flat, ts));
        }

        None
    }
}

/// 解析 payload（组合流的 data 字段或单流的顶层）
pub(crate) fn parse_payload(
    v: &serde_json::Value,
) -> Option<(Vec<[String; 2]>, Vec<[String; 2]>, Option<String>, i64)> {
    // Spot format: bids/asks
    if let (Some(bids), Some(asks)) = (v.get("bids"), v.get("asks")) {
        let bids = parse_levels(bids)?;
        let asks = parse_levels(asks)?;
        let ts = v.get("lastUpdateId").and_then(|t| t.as_i64()).unwrap_or_else(|| {
            tracing::warn!("orderbook_ws: lastUpdateId missing in spot payload — using 0 as fallback");
            0
        });
        return Some((bids, asks, None, ts));
    }
    // Perpetual format: b/a
    if let (Some(bids), Some(asks)) = (v.get("b"), v.get("a")) {
        let bids = parse_levels(bids)?;
        let asks = parse_levels(asks)?;
        let sym = v.get("s").and_then(|s| s.as_str()).map(String::from);
        let ts = v.get("E").and_then(|t| t.as_i64()).unwrap_or_else(|| {
            tracing::warn!("orderbook_ws: event time 'E' missing in perpetual payload — using 0 as fallback");
            0
        });
        return Some((bids, asks, sym, ts));
    }
    None
}

pub(crate) fn parse_levels(v: &serde_json::Value) -> Option<Vec<[String; 2]>> {
    let arr = v.as_array()?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let pair = item.as_array()?;
        if pair.len() < 2 {
            return None;
        }
        let p = pair[0].as_str().or_else(|| pair[0].as_f64().map(|_| ""))?;
        let a = pair[1].as_str().or_else(|| pair[1].as_f64().map(|_| ""))?;
        // If numbers, convert to string via serde_json
        let p = if p.is_empty() {
            pair[0].as_f64().map(|n| n.to_string())?
        } else {
            p.to_string()
        };
        let a = if a.is_empty() {
            pair[1].as_f64().map(|n| n.to_string())?
        } else {
            a.to_string()
        };
        out.push([p, a]);
    }
    Some(out)
}

pub(crate) fn to_levels(raw: &[[String; 2]]) -> Vec<OrderBookLevel> {
    raw.iter()
        .filter_map(|[p, a]| {
            let price: f64 = p.parse().ok()?;
            let amount: f64 = a.parse().ok()?;
            if amount > 0.0 {
                Some(OrderBookLevel { price, amount })
            } else {
                None
            }
        })
        .collect()
}

enum WsCommand {
    Subscribe(String),
    Unsubscribe(String),
}

pub struct OrderBookWs {
    ws_url: String,
    reconnect_delay_secs: u64,
    max_reconnect_delay_secs: u64,
    ws_ping_interval_secs: u64,
    ws_max_lifetime_secs: u64,
    subscriptions: Arc<RwLock<Vec<String>>>,
    /// Map: lowercase binance symbol (e.g. "btcusdt") → unified symbol (e.g. "BTC/USDT")
    symbol_map: Arc<RwLock<HashMap<String, String>>>,
    running: Arc<AtomicBool>,
    request_id: Arc<AtomicU64>,
    shutdown_tx: Option<mpsc::Sender<()>>,
    command_tx: Option<mpsc::UnboundedSender<WsCommand>>,
}

impl OrderBookWs {
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
        // Use /stream endpoint to get wrapped messages {"stream":..., "data":...}
        // This is required because spot partial book depth payloads do NOT include
        // the symbol — we must extract it from the stream name.
        Self::new(
            "wss://stream.binance.com/stream".to_string(),
            1,
            60,
            30,
            23 * 3600,
        )
    }

    pub fn new_perpetual(_proxy_url: Option<&str>) -> Self {
        // Use /public/stream endpoint for consistency — perpetual payloads include `s` field,
        // but using /stream simplifies symbol resolution for both market types.
        // 2026-04-23 起币安将公共高频流量切流至 /public 路由（depth/aggTrade/trade）
        Self::new(
            "wss://fstream.binance.com/public/stream".to_string(),
            1,
            60,
            30,
            23 * 3600,
        )
    }
}

#[async_trait]
impl OrderBookWsClient for OrderBookWs {
    async fn start(&mut self, update_tx: broadcast::Sender<WsOrderBookEvent>) {
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
                            if update_tx.send(WsOrderBookEvent::Reconnected).is_err() {
                                tracing::warn!("[OrderBookWs] All receivers dropped, stopping");
                                running.store(false, Ordering::Relaxed);
                                break;
                            }
                        }
                        is_first_connect = false;

                        let (mut write, mut read) = ws_stream.split();

                        // Re-subscribe existing streams on (re)connect
                        {
                            // Clone subscriptions and drop the read guard before
                            // async send to avoid holding the lock across .await
                            let subs_vec: Vec<String> = subscriptions.read().await.clone();
                            if !subs_vec.is_empty() {
                                let id = request_id.fetch_add(1, Ordering::Relaxed);
                                let msg = serde_json::json!({
                                    "method": "SUBSCRIBE",
                                    "params": subs_vec,
                                    "id": id
                                });
                                if let Ok(text) = serde_json::to_string(&msg) {
                                    if write
                                        .send(tungstenite::Message::Text(text.into()))
                                        .await
                                        .is_err()
                                    {
                                        tracing::error!(
                                            "[OrderBookWs] Failed to send resubscribe"
                                        );
                                        continue;
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
                                            if let Ok(bmsg) = serde_json::from_str::<BinanceDepthMessage>(&text) {
                                                if let Some((bids_raw, asks_raw, stream_name, sym_from_payload, ts)) = bmsg.into_depth() {
                                                    // Resolve unified symbol:
                                                    // 1. Try stream name (e.g. "btcusdt@depth20@500ms" → "btcusdt")
                                                    // 2. Fall back to payload symbol (perpetual `s` field)
                                                    // 3. Fall back to single-subscription map lookup
                                                    let original_symbol = resolve_symbol(
                                                        stream_name.as_deref(),
                                                        sym_from_payload.as_deref(),
                                                        &symbol_map,
                                                    ).await;

                                                    if let Some(symbol) = original_symbol {
                                                        let bids = to_levels(&bids_raw);
                                                        let asks = to_levels(&asks_raw);
                                                        if !bids.is_empty() || !asks.is_empty() {
                                                            if update_tx.send(WsOrderBookEvent::OrderBook(
                                                                WsOrderBookUpdate {
                                                                    symbol,
                                                                    bids,
                                                                    asks,
                                                                    timestamp: ts,
                                                                }
                                                            )).is_err() {
                                                                tracing::warn!("[OrderBookWs] All receivers dropped, stopping");
                                                                running.store(false, Ordering::Relaxed);
                                                                break;
                                                            }
                                                        }
                                                    }
                                                } else {
                                                }
                                            } else {
                                                tracing::warn!(
                                                    "[OrderBookWs] Failed to parse: {}",
                                                    &text[..text.len().min(200)]
                                                );
                                            }
                                        }
                                        Some(Ok(tungstenite::Message::Ping(data))) => {
                                            let _ = write.send(tungstenite::Message::Pong(data)).await;
                                        }
                                        Some(Ok(tungstenite::Message::Close(_))) => {
                                            tracing::warn!("[OrderBookWs] Server closed");
                                            break;
                                        }
                                        Some(Err(e)) => {
                                            tracing::error!("[OrderBookWs] Read error: {}", e);
                                            break;
                                        }
                                        None => {
                                            tracing::warn!("[OrderBookWs] Stream ended");
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                                _ = ping_tick.tick() => {
                                    let ping = tungstenite::Message::Ping(vec![].into());
                                    if write.send(ping).await.is_err() {
                                        tracing::warn!("[OrderBookWs] Ping failed");
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
                                                    tracing::warn!("[OrderBookWs] Subscribe send failed");
                                                    break;
                                                }
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
                                                    tracing::warn!("[OrderBookWs] Unsubscribe send failed");
                                                    break;
                                                }
                                            }
                                        }
                                        None => break,
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
                        tracing::error!("[OrderBookWs] Connection failed: {}", e);
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
        let stream_name = format!("{}@depth20@500ms", binance_ws_symbol(symbol));
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
        let stream_name = format!("{}@depth20@500ms", binance_ws_symbol(symbol));
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

/// Resolve unified symbol from stream name, payload symbol, or symbol_map.
///
/// Resolution order:
/// 1. Stream name (e.g. "btcusdt@depth20@500ms" → "btcusdt" → lookup in map)
/// 2. Payload symbol (perpetual `s` field, e.g. "BTCUSDT" → lowercase → lookup)
/// 3. Single-subscription fallback (if only one symbol subscribed)
async fn resolve_symbol(
    stream_name: Option<&str>,
    sym_from_payload: Option<&str>,
    symbol_map: &Arc<RwLock<HashMap<String, String>>>,
) -> Option<String> {
    let map = symbol_map.read().await;

    // 1. Try stream name: "btcusdt@depth20@500ms" → "btcusdt"
    if let Some(stream) = stream_name {
        if let Some(symbol_part) = stream.split('@').next() {
            let key = symbol_part.to_lowercase();
            if let Some(unified) = map.get(&key) {
                return Some(unified.clone());
            }
        }
    }

    // 2. Try payload symbol (perpetual `s` field)
    if let Some(s) = sym_from_payload {
        let key = s.to_lowercase();
        if let Some(unified) = map.get(&key) {
            return Some(unified.clone());
        }
    }

    // 3. Single-subscription fallback (spot raw payload, no stream name)
    if map.len() == 1 {
        return map.values().next().cloned();
    }

    None
}
