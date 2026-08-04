use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use tokio::sync::{broadcast, mpsc, RwLock};

use virs_ws::{
    ConnectionReason, MessageOutcome, WsCommand as ManagerWsCommand, WsHandler, WsManager,
    WsManagerConfig, WsManagerEvent,
};
use crate::ws_types::KlineWsClient;
pub use crate::ws_types::{WsCandleUpdate, WsEvent};
use virs_types::Candle;
use virs_task::{spawn, Stop, TaskHandle};

pub(crate) fn binance_ws_symbol(symbol: &str) -> String {
    symbol.replace('/', "").to_lowercase()
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BinanceKlineMessage {
    #[allow(dead_code)]
    pub(crate) stream: Option<String>,

    pub(crate) data: Option<BinanceKlineData>,

    #[serde(rename = "e")]
    pub(crate) event_type_flat: Option<String>,

    #[serde(rename = "E")]
    pub(crate) event_time_flat: Option<i64>,
    #[serde(rename = "s")]
    pub(crate) symbol_flat: Option<String>,
    #[serde(rename = "k")]
    pub(crate) kline_flat: Option<BinanceKlineInner>,
}

impl BinanceKlineMessage {
    pub(crate) fn into_kline_data(self) -> Option<BinanceKlineData> {
        if let Some(data) = self.data {
            Some(data)
        } else if let Some(et) = self.event_type_flat.as_deref() {
            if et == "kline" {
                if self.symbol_flat.is_none() {
                    tracing::warn!("Kline WS message missing symbol — skipping kline");
                    return None;
                }

                let event_time = self.event_time_flat?;
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
                tracing::error!(symbol = %symbol, field = %field, raw = %raw, error = %e, "Failed to parse kline OHLCV field — returning NoData instead of 0.0");
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

pub struct KlineWsHandler {
    ws_url: String,
    pub(crate) subscriptions: Arc<RwLock<Vec<String>>>,
    pub(crate) symbol_map: Arc<RwLock<HashMap<String, String>>>,
    request_id: Arc<AtomicU64>,
}

impl KlineWsHandler {
    pub fn new(ws_url: String) -> Self {
        Self {
            ws_url,
            subscriptions: Arc::new(RwLock::new(Vec::new())),
            symbol_map: Arc::new(RwLock::new(HashMap::new())),
            request_id: Arc::new(AtomicU64::new(1)),
        }
    }
}

#[async_trait]
impl WsHandler<WsEvent> for KlineWsHandler {
    fn base_url(&self) -> &str {
        &self.ws_url
    }

    fn supports_commands(&self) -> bool {
        true
    }

    async fn on_message(
        &self,
        text: &str,
    ) -> Result<MessageOutcome<WsEvent>, virs_error::VirsError> {
        let bmsg: BinanceKlineMessage = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(_) => {
                let preview: String = text.chars().take(200).collect();
                tracing::warn!(
                    preview = %preview,
                    "Failed to parse WS message"
                );
                return Ok(MessageOutcome::Continue(vec![]));
            }
        };

        if let Some(data) = bmsg.into_kline_data() {
            if data.event_type == "kline" {
                if data.event_time > 0 {
                    let local_now = chrono::Utc::now().timestamp_millis();
                    let delay_ms = local_now - data.event_time;
                    if delay_ms > KLINE_WS_DELAY_THRESHOLD_MS {
                        tracing::warn!(
                            delay_ms = delay_ms,
                            event_time = data.event_time,
                            local_time = local_now,
                            symbol = %data.kline.symbol,
                            "Message delay exceeds threshold"
                        );
                    }
                }

                let raw_sym = data.ws_symbol().to_lowercase();
                let original_symbol = {
                    let map = self.symbol_map.read().await;
                    map.get(&raw_sym).cloned()
                };
                let original_symbol = match original_symbol {
                    Some(s) => s,
                    None => {
                        tracing::warn!(
                            ws_symbol = %data.ws_symbol(),
                            "symbol_map miss — skipping kline update (symbol not in subscription map)"
                        );
                        return Ok(MessageOutcome::Continue(vec![]));
                    }
                };

                let candle = match data.to_candle() {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!(
                            symbol = %data.ws_symbol(),
                            error = %e,
                            "Failed to parse kline — skipping this candle update"
                        );
                        return Ok(MessageOutcome::Continue(vec![]));
                    }
                };

                return Ok(MessageOutcome::Continue(vec![WsEvent::Candle(
                    WsCandleUpdate {
                        symbol: original_symbol,
                        candle,
                    },
                )]));
            }
        } else {
            if let Ok(resp) = serde_json::from_str::<serde_json::Value>(text) {
                if let Some(code) = resp.get("code") {
                    tracing::error!(
                        id = ?resp.get("id"),
                        code = ?code,
                        msg = ?resp.get("msg"),
                        "Subscription rejected by Binance"
                    );
                } else if resp.get("result").is_some() {
                    tracing::info!(
                        id = ?resp.get("id"),
                        "Subscription confirmed by Binance"
                    );
                }
            }
        }

        Ok(MessageOutcome::Continue(vec![]))
    }

    async fn on_connected(&self, _is_reconnect: bool) -> Vec<String> {
        let subs_vec = self.subscriptions.read().await.clone();
        if subs_vec.is_empty() {
            return vec![];
        }

        let id = self.request_id.fetch_add(1, Ordering::Relaxed);
        let msg = serde_json::json!({
            "method": "SUBSCRIBE",
            "params": subs_vec,
            "id": id
        });

        tracing::info!(
            id = id,
            count = subs_vec.len(),
            "Batch subscription request sent on connect"
        );

        vec![msg.to_string()]
    }

    async fn on_disconnected(&self) {}

    async fn on_command(&self, cmd: ManagerWsCommand) -> Option<String> {
        let (method, stream_name) = match cmd {
            ManagerWsCommand::Subscribe(s) => ("SUBSCRIBE", s),
            ManagerWsCommand::Unsubscribe(s) => ("UNSUBSCRIBE", s),
        };

        let id = self.request_id.fetch_add(1, Ordering::Relaxed);
        let msg = serde_json::json!({
            "method": method,
            "params": [stream_name.clone()],
            "id": id
        });

        tracing::info!(
            id = id,
            method = %method,
            stream = %stream_name,
            "Dynamic subscription request sent"
        );

        Some(msg.to_string())
    }
}

pub struct KlineWs {
    manager: WsManager<WsEvent>,
    config: WsManagerConfig,
    pub(crate) handler: Arc<KlineWsHandler>,
    forward_task: std::sync::Mutex<Option<TaskHandle>>,
}

impl KlineWs {
    pub fn new(ws_url: String) -> Self {
        let handler = Arc::new(KlineWsHandler::new(ws_url));

        Self {
            manager: WsManager::new(handler.clone()),
            config: WsManagerConfig::default(),
            handler,
            forward_task: std::sync::Mutex::new(None),
        }
    }

    pub fn new_perpetual(_proxy_url: Option<&str>) -> Self {
        Self::new("wss://fstream.binance.com/market/ws".to_string())
    }

    pub fn running_handle(&self) -> Arc<AtomicBool> {
        self.manager.running_handle()
    }
}

#[async_trait]
impl KlineWsClient for KlineWs {
    async fn start(&mut self, update_tx: broadcast::Sender<WsEvent>) {
        let (manager_tx, mut manager_rx) = mpsc::channel::<WsManagerEvent<WsEvent>>(256);

        self.manager
            .start(self.config.clone(), manager_tx)
            .await;

        let handle = spawn("kline_forward", move |stop: Stop| async move {
            loop {
                tokio::select! {
                    _ = stop.cancelled() => break,
                    ev = manager_rx.recv() => {
                        let Some(ev) = ev else { break };
                        let ws_event = match ev {
                            WsManagerEvent::Message(e) => e,
                            WsManagerEvent::ConnectionChanged {
                                connected: true,
                                reason: ConnectionReason::Reconnected,
                            } => WsEvent::Reconnected,
                            WsManagerEvent::ConnectionChanged {
                                connected: true,
                                ..
                            } => {
                                continue;
                            }
                            WsManagerEvent::ConnectionChanged {
                                connected: false, ..
                            } => {
                                continue;
                            }
                            WsManagerEvent::CircuitBreakerTripped { retry_count } => {
                                tracing::error!(
                                    retry_count = retry_count,
                                    "Circuit breaker tripped — WS stopped after max retries"
                                );
                                continue;
                            }
                        };

                        if update_tx.send(ws_event).is_err() {
                            tracing::warn!("All receivers dropped, stopping forwarder");
                            break;
                        }
                    }
                }
            }
        });

        *self.forward_task.lock().unwrap() = Some(handle);
    }

    async fn stop(&mut self) {
        self.manager.stop().await;
        let handle = self.forward_task.lock().unwrap().take();
        if let Some(h) = handle {
            h.cancel();
            h.join().await;
        }
    }

    async fn subscribe(&self, symbol: &str) {
        let stream_name = format!("{}@kline_1m", binance_ws_symbol(symbol));
        let ws_sym = binance_ws_symbol(symbol);

        {
            let mut map = self.handler.symbol_map.write().await;
            map.insert(ws_sym, symbol.to_string());
        }

        let mut subs = self.handler.subscriptions.write().await;
        if !subs.contains(&stream_name) {
            subs.push(stream_name.clone());
            drop(subs);
            self.manager
                .send_command(ManagerWsCommand::Subscribe(stream_name))
                .await;
        }
    }

    async fn unsubscribe(&self, symbol: &str) {
        let stream_name = format!("{}@kline_1m", binance_ws_symbol(symbol));
        let ws_sym = binance_ws_symbol(symbol);

        {
            let mut map = self.handler.symbol_map.write().await;
            map.remove(&ws_sym);
        }

        let mut subs = self.handler.subscriptions.write().await;
        let existed = subs.iter().any(|s| s == &stream_name);
        subs.retain(|s| s != &stream_name);
        drop(subs);
        if existed {
            self.manager
                .send_command(ManagerWsCommand::Unsubscribe(stream_name))
                .await;
        }
    }

    fn is_running(&self) -> bool {
        self.manager.is_running()
    }
}
