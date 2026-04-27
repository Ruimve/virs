use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_tungstenite::{connect_async, tungstenite};
use tracing;

use super::types::{Candle, KlineWsClient, WsCandleUpdate, WsEvent};

fn binance_ws_symbol(symbol: &str) -> String {
    symbol.replace('/', "").to_lowercase()
}

#[derive(Debug, Clone, Deserialize)]
struct BinanceKlineMessage {
    #[allow(dead_code)]
    stream: Option<String>,
    data: Option<BinanceKlineData>,
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

pub struct BinanceWs {
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

impl BinanceWs {
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

    pub fn new_spot(proxy_url: Option<&str>) -> Self {
        let _ = proxy_url;
        Self::new(
            "wss://stream.binance.com/ws".to_string(),
            1, 60, 30, 23 * 3600,
        )
    }

    pub fn new_perpetual(proxy_url: Option<&str>) -> Self {
        let _ = proxy_url;
        Self::new(
            "wss://fstream.binance.com/ws".to_string(),
            1, 60, 30, 23 * 3600,
        )
    }

    pub async fn subscription_count(&self) -> usize {
        self.subscriptions.lock().await.len()
    }
}

#[async_trait]
impl KlineWsClient for BinanceWs {
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

                tracing::info!("[KlineWs] Connecting to {}...", ws_url);

                match connect_async(&ws_url).await {
                    Ok((ws_stream, _)) => {
                        tracing::info!("[KlineWs] Connected successfully");
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
                                let msg = serde_json::json!({
                                    "method": "SUBSCRIBE",
                                    "params": subs_vec,
                                    "id": id
                                });
                                if let Ok(text) = serde_json::to_string(&msg) {
                                    if write.send(tungstenite::Message::Text(text.into())).await.is_err() {
                                        tracing::error!("[KlineWs] Failed to send subscription message");
                                        continue;
                                    }
                                }
                                tracing::info!("[KlineWs] Subscribed to {} streams", subs.len());
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
                                tracing::info!("[KlineWs] Max lifetime reached, reconnecting...");
                                break;
                            }

                            tokio::select! {
                                msg = read.next() => {
                                    match msg {
                                        Some(Ok(tungstenite::Message::Text(text))) => {
                                            if let Ok(bmsg) = serde_json::from_str::<BinanceKlineMessage>(&text) {
                                                if let Some(data) = bmsg.data {
                                                    if data.event_type == "kline" {
                                                        let raw_sym = data.ws_symbol().to_lowercase();
                                                        let original_symbol = {
                                                            let map = symbol_map.lock().await;
                                                            map.get(&raw_sym).cloned().unwrap_or_else(|| raw_sym.clone())
                                                        };
                                                        let candle = data.to_candle();
                                                        let _ = update_tx.send(WsEvent::Candle(WsCandleUpdate {
                                                            symbol: original_symbol,
                                                            candle,
                                                        }));
                                                    }
                                                }
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
                                                "params": [stream_name],
                                                "id": id
                                            });
                                            if let Ok(text) = serde_json::to_string(&msg) {
                                                if write.send(tungstenite::Message::Text(text.into())).await.is_err() {
                                                    tracing::warn!("[KlineWs] Failed to send dynamic subscribe");
                                                    break;
                                                }
                                                tracing::info!("[KlineWs] Dynamically subscribed to stream");
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
                                                    tracing::warn!("[KlineWs] Failed to send dynamic unsubscribe");
                                                    break;
                                                }
                                                tracing::info!("[KlineWs] Dynamically unsubscribed from stream");
                                            }
                                        }
                                        None => {
                                            tracing::info!("[KlineWs] Command channel closed");
                                            break;
                                        }
                                    }
                                }
                                _ = shutdown_rx.recv() => {
                                    tracing::info!("[KlineWs] Shutdown requested");
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

                tracing::info!("[KlineWs] Reconnecting in {}s...", reconnect_delay);
                tokio::time::sleep(Duration::from_secs(reconnect_delay)).await;
                reconnect_delay = (reconnect_delay * 2).min(max_reconnect_delay_secs);
            }

            running.store(false, Ordering::Relaxed);
            tracing::info!("[KlineWs] Worker exited");
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
