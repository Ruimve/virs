use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use tokio::sync::{broadcast, mpsc, RwLock};

use virs_ws::{
    ConnectionReason, MessageOutcome, WsCommand as ManagerWsCommand, WsHandler, WsManager,
    WsManagerConfig, WsManagerEvent,
};
use crate::ws_types::{OrderBookLevel, OrderBookWsClient, WsOrderBookEvent, WsOrderBookUpdate};
use virs_runtime::{CancellationToken, TaskSupervisor};

fn binance_ws_symbol(symbol: &str) -> String {
    symbol.replace('/', "").to_lowercase()
}

pub(crate) const ORDERBOOK_WS_DELAY_THRESHOLD_MS: i64 = 2_000;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BinanceDepthMessage {
    stream: Option<String>,

    data: Option<serde_json::Value>,

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

pub(crate) struct ParsedDepth {
    pub bids: Vec<[String; 2]>,
    pub asks: Vec<[String; 2]>,
    pub stream_name: Option<String>,
    pub symbol: Option<String>,
    pub timestamp_ms: i64,
    pub last_update_id: Option<i64>,
}

impl BinanceDepthMessage {
    pub(crate) fn into_depth(self) -> Option<ParsedDepth> {
        let stream = self.stream.clone();

        if let Some(data) = self.data {
            if let Some(pd) = parse_payload(&data) {
                return Some(ParsedDepth {
                    stream_name: stream,
                    ..pd
                });
            }
            return None;
        }

        if let (Some(bids), Some(asks)) = (self.bids_perp_flat, self.asks_perp_flat) {
            let ts = self.event_time_flat?;
            return Some(ParsedDepth {
                bids,
                asks,
                stream_name: stream,
                symbol: self.symbol_flat,
                timestamp_ms: ts,
                last_update_id: None,
            });
        }

        None
    }
}

pub(crate) fn parse_payload(v: &serde_json::Value) -> Option<ParsedDepth> {
    if let (Some(bids), Some(asks)) = (v.get("b"), v.get("a")) {
        let bids = parse_levels(bids)?;
        let asks = parse_levels(asks)?;
        let sym = v.get("s").and_then(|s| s.as_str()).map(String::from);
        let ts = v.get("E").and_then(|t| t.as_i64())?;
        return Some(ParsedDepth {
            bids,
            asks,
            stream_name: None,
            symbol: sym,
            timestamp_ms: ts,
            last_update_id: None,
        });
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

pub struct OrderBookWsHandler {
    ws_url: String,
    pub(crate) subscriptions: Arc<RwLock<Vec<String>>>,
    pub(crate) symbol_map: Arc<RwLock<HashMap<String, String>>>,
    request_id: Arc<AtomicU64>,
}

impl OrderBookWsHandler {
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
impl WsHandler<WsOrderBookEvent> for OrderBookWsHandler {
    fn base_url(&self) -> &str {
        &self.ws_url
    }

    fn supports_commands(&self) -> bool {
        true
    }

    async fn on_message(
        &self,
        text: &str,
    ) -> Result<MessageOutcome<WsOrderBookEvent>, virs_error::VirsError> {
        let bmsg: BinanceDepthMessage = match serde_json::from_str(text) {
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

        if let Some(pd) = bmsg.into_depth() {
            if pd.timestamp_ms > 0 {
                let local_now = chrono::Utc::now().timestamp_millis();
                let delay_ms = local_now - pd.timestamp_ms;
                if delay_ms > ORDERBOOK_WS_DELAY_THRESHOLD_MS {
                    tracing::warn!(
                        delay_ms = delay_ms,
                        event_time = pd.timestamp_ms,
                        local_time = local_now,
                        "Message delay exceeds threshold"
                    );
                }
            }

            let original_symbol = resolve_symbol(
                pd.stream_name.as_deref(),
                pd.symbol.as_deref(),
                &self.symbol_map,
            )
            .await;

            if let Some(symbol) = original_symbol {
                let bids = to_levels(&pd.bids);
                let asks = to_levels(&pd.asks);
                if !bids.is_empty() || !asks.is_empty() {
                    return Ok(MessageOutcome::Continue(vec![WsOrderBookEvent::OrderBook(
                        WsOrderBookUpdate {
                            symbol,
                            bids,
                            asks,
                            timestamp: pd.timestamp_ms,
                            last_update_id: pd.last_update_id,
                        },
                    )]));
                }
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

pub struct OrderBookWs {
    manager: WsManager<WsOrderBookEvent>,
    config: WsManagerConfig,
    pub(crate) handler: Arc<OrderBookWsHandler>,
    supervisor: TaskSupervisor,
}

impl OrderBookWs {
    pub fn new(ws_url: String, parent_cancel: CancellationToken) -> Self {
        let handler = Arc::new(OrderBookWsHandler::new(ws_url));

        Self {
            manager: WsManager::new(handler.clone(), parent_cancel.clone()),
            config: WsManagerConfig::default(),
            handler,
            supervisor: TaskSupervisor::new(parent_cancel.child_token()),
        }
    }

    pub fn new_perpetual(_proxy_url: Option<&str>, parent_cancel: CancellationToken) -> Self {
        Self::new("wss://fstream.binance.com/public/stream".to_string(), parent_cancel)
    }

    pub fn running_handle(&self) -> Arc<AtomicBool> {
        self.manager.running_handle()
    }
}

#[async_trait]
impl OrderBookWsClient for OrderBookWs {
    async fn start(&mut self, update_tx: broadcast::Sender<WsOrderBookEvent>) {
        let (manager_tx, mut manager_rx) = mpsc::channel::<WsManagerEvent<WsOrderBookEvent>>(256);

        self.manager
            .start(self.config.clone(), manager_tx)
            .await;

        let ws_cancel = self
            .manager
            .cancellation_token()
            .await
            .unwrap_or_else(|| self.supervisor.child_token());

        self.supervisor
            .spawn_raw("orderbook_forward", move |supervisor_cancel| async move {
                loop {
                    tokio::select! {
                        _ = supervisor_cancel.cancelled() => break,
                        _ = ws_cancel.cancelled() => break,
                        ev = manager_rx.recv() => {
                            let Some(ev) = ev else { break };
                            let ws_event = match ev {
                                WsManagerEvent::Message(e) => e,
                                WsManagerEvent::ConnectionChanged {
                                    connected: true,
                                    reason: ConnectionReason::Reconnected,
                                } => WsOrderBookEvent::Reconnected,
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
            })
            .await;
    }

    async fn stop(&mut self) {
        self.manager.stop().await;
        self.supervisor.shutdown().await;
    }

    async fn subscribe(&self, symbol: &str) {
        let stream_name = format!("{}@depth20@500ms", binance_ws_symbol(symbol));
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
        let stream_name = format!("{}@depth20@500ms", binance_ws_symbol(symbol));
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

async fn resolve_symbol(
    stream_name: Option<&str>,
    sym_from_payload: Option<&str>,
    symbol_map: &Arc<RwLock<HashMap<String, String>>>,
) -> Option<String> {
    let map = symbol_map.read().await;

    if let Some(stream) = stream_name {
        if let Some(symbol_part) = stream.split('@').next() {
            let key = symbol_part.to_lowercase();
            if let Some(unified) = map.get(&key) {
                return Some(unified.clone());
            }
        }
    }

    if let Some(s) = sym_from_payload {
        let key = s.to_lowercase();
        if let Some(unified) = map.get(&key) {
            return Some(unified.clone());
        }
    }

    if map.len() == 1 {
        return map.values().next().cloned();
    }

    None
}
