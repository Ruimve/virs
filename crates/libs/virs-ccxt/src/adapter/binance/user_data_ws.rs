use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use serde::Deserialize;
use tokio::sync::mpsc;
use virs_error::ExchangeError;


pub use virs_types::WsFeedEvent;
use virs_types::{OrderStatus, PositionSide};

use crate::adapter::binance::fapi;
use crate::adapter::binance::user_data_ws_events::dispatch_event;
use crate::auth::Signer;
use crate::ws_manager::{MessageOutcome, WsHandler, WsManager, WsManagerConfig, WsManagerEvent};
use crate::ExchangeClient;


#[derive(Debug, Clone, Deserialize)]
pub struct BinanceOrderMessage {
    #[allow(dead_code)]
    pub(crate) stream: Option<String>,

    pub(crate) data: Option<BinanceOrderData>,

    #[serde(rename = "e")]
    pub(crate) event_type_flat: Option<String>,

    #[serde(rename = "E")]
    event_time_flat: Option<i64>,
    #[serde(rename = "o")]
    order_flat: Option<BinanceOrderInner>,
}

impl BinanceOrderMessage {

    pub fn event_type(&self) -> Option<&str> {
        self.event_type_flat
            .as_deref()
            .or_else(|| self.data.as_ref().map(|d| d.event_type.as_str()))
    }


    pub fn event_time(&self) -> Option<i64> {
        self.event_time_flat
            .or_else(|| self.data.as_ref().map(|d| d.event_time))
    }


    pub fn to_ws_feed_event(self) -> Option<WsFeedEvent> {

        if let Some(et) = self.event_type_flat.as_deref() {
            if et == "ORDER_TRADE_UPDATE" {

                if let Some(order) = self.order_flat {
                    return order.to_ws_feed_event();
                }
            }
        }

        if let Some(data) = self.data {
            if data.event_type == "ORDER_TRADE_UPDATE" {
                return data.order.to_ws_feed_event();
            }
        }
        None
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct BinanceOrderData {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: i64,
    #[serde(rename = "o")]
    pub order: BinanceOrderInner,
}


#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct BinanceOrderInner {

    #[serde(rename = "s")]
    pub(crate) symbol: String,

    #[serde(rename = "c")]
    pub(crate) client_order_id: String,

    #[serde(rename = "S")]
    pub(crate) side: String,

    #[serde(rename = "o")]
    pub(crate) order_type: String,

    #[serde(rename = "X")]
    pub(crate) status: String,

    #[serde(rename = "i")]
    pub(crate) order_id: i64,

    #[serde(rename = "q")]
    pub(crate) orig_qty: String,

    #[serde(rename = "z")]
    pub(crate) filled_qty: String,

    #[serde(rename = "Q")]
    pub(crate) remaining_qty: Option<String>,

    #[serde(rename = "L")]
    pub(crate) last_fill_price: String,

    #[serde(rename = "ap")]
    pub(crate) avg_fill_price: Option<String>,

    #[serde(rename = "l")]
    pub(crate) last_fill_qty: String,

    #[serde(rename = "n")]
    pub(crate) commission: String,

    #[serde(rename = "N")]
    pub(crate) commission_asset: String,

    #[serde(rename = "T")]
    pub(crate) trade_time: i64,

    #[serde(rename = "R")]
    pub(crate) is_reduce_only: bool,

    #[serde(rename = "w")]
    pub(crate) working_type: String,

    #[serde(rename = "ps")]
    pub(crate) position_side: Option<String>,
}

impl BinanceOrderInner {

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
                            symbol = %self.symbol,
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
            client_order_id: Some(self.client_order_id.clone()),
            symbol: self.symbol.clone(),
            status,
            filled,
            remaining,
            price,
            amount,
            commission,
            timestamp: DateTime::from_timestamp_millis(self.trade_time).unwrap_or_else(|| {
                tracing::warn!(
                    trade_time = self.trade_time,
                    symbol = %self.symbol,
                    order_id = %self.order_id,
                    "WS order trade_time invalid — using local time as fallback"
                );
                Utc::now()
            }),
            position_side,
        })
    }
}


pub(crate) const ORDER_WS_DELAY_THRESHOLD_MS: i64 = 3_000;


pub struct UserDataWsHandler {

    ws_url: String,

    client: ExchangeClient,

    signer: Arc<dyn Signer>,

    current_key: Arc<RwLock<String>>,
}

impl UserDataWsHandler {


    pub fn new(
        ws_url: String,
        client: ExchangeClient,
        signer: Arc<dyn Signer>,
        current_key: Arc<RwLock<String>>,
    ) -> Self {
        Self {
            ws_url,
            client,
            signer,
            current_key,
        }
    }
}

#[async_trait::async_trait]
impl WsHandler<WsFeedEvent> for UserDataWsHandler {
    fn base_url(&self) -> &str {
        &self.ws_url
    }

    async fn refresh_url(&self) -> Result<String, ExchangeError> {

        let new_key = fapi::create_listen_key(&self.client, self.signer.as_ref()).await?;

        *self.current_key
            .write()
            .expect("listenKey RwLock poisoned") = new_key.clone();
        let url = format!("wss://fstream.binance.com/private/ws?listenKey={}", new_key);
        tracing::info!("[UserDataWs] Refreshed listenKey for reconnect");
        Ok(url)
    }

    async fn on_message(
        &self,
        text: &str,
    ) -> Result<MessageOutcome<WsFeedEvent>, ExchangeError> {

        if let Ok(bmsg) = serde_json::from_str::<BinanceOrderMessage>(text) {
            if let Some(et) = bmsg.event_time() {
                if et > 0 {
                    let delay_ms = chrono::Utc::now().timestamp_millis() - et;
                    if delay_ms > ORDER_WS_DELAY_THRESHOLD_MS {
                        let event_type = bmsg
                            .event_type()
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "unknown".to_string());
                        tracing::warn!(
                            delay_ms = delay_ms,
                            event_time = et,
                            event_type = %event_type,
                            "[UserDataWs] Order event delay exceeds threshold"
                        );
                    }
                }
            }
        }


        if let Some(event) = dispatch_event(text) {
            return Ok(MessageOutcome::Continue(vec![event]));
        }


        if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
            let payload = value.get("data").unwrap_or(&value);
            if let Some(et) = payload.get("e").and_then(|v| v.as_str()) {
                if et == "listenKeyExpired" {
                    tracing::warn!(
                        "[UserDataWs] listenKey expired — requesting reconnect with fresh key"
                    );
                    return Ok(MessageOutcome::Reconnect);
                }
                if et == "serverShutdown" {
                    tracing::warn!("[UserDataWs] Server shutdown event — requesting reconnect");
                    return Ok(MessageOutcome::Reconnect);
                }
            }
        }


        Ok(MessageOutcome::Continue(vec![]))
    }

    async fn on_connected(&self, _is_reconnect: bool) -> Vec<String> {

        vec![]
    }

    async fn on_disconnected(&self) {

    }
}


pub struct UserDataWs {
    manager: WsManager<WsFeedEvent>,

    pub ws_url: String,

    current_key: Arc<RwLock<String>>,
}

impl UserDataWs {


    pub fn new_perpetual(
        listen_key: String,
        client: ExchangeClient,
        signer: Arc<dyn Signer>,
    ) -> Self {
        let base_url = "wss://fstream.binance.com/private/ws".to_string();
        let ws_url = format!("{}?listenKey={}", base_url, listen_key);

        let current_key = Arc::new(RwLock::new(listen_key));

        let handler = Arc::new(UserDataWsHandler::new(
            ws_url.clone(),
            client,
            signer,
            Arc::clone(&current_key),
        ));

        let config = WsManagerConfig::default();

        Self {
            manager: WsManager::new(config, handler),
            ws_url,
            current_key,
        }
    }


    pub fn running_handle(&self) -> std::sync::Arc<std::sync::atomic::AtomicBool> {
        self.manager.running_handle()
    }


    pub fn listen_key_handle(&self) -> Arc<RwLock<String>> {
        Arc::clone(&self.current_key)
    }


    pub async fn start(&self, event_tx: mpsc::Sender<WsFeedEvent>) {

        let (manager_tx, mut manager_rx) = mpsc::channel::<WsManagerEvent<WsFeedEvent>>(256);


        self.manager.start(manager_tx).await;


        tokio::spawn(async move {
            while let Some(ev) = manager_rx.recv().await {
                let feed_event = match ev {
                    WsManagerEvent::Message(e) => e,
                    WsManagerEvent::ConnectionChanged { connected, .. } => {
                        WsFeedEvent::ConnectionChanged { connected }
                    }
                    WsManagerEvent::CircuitBreakerTripped { retry_count } => {
                        tracing::error!(
                            retry_count = retry_count,
                            "[UserDataWs] Circuit breaker tripped — WS stopped after max retries"
                        );
                        WsFeedEvent::ConnectionChanged { connected: false }
                    }
                };
                if event_tx.send(feed_event).await.is_err() {
                    tracing::warn!("[UserDataWs] External event channel closed, stopping forwarder");
                    break;
                }
            }
        });
    }


    pub async fn stop(&self) {
        self.manager.stop().await;
    }
}
