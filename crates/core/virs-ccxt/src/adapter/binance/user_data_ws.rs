use std::sync::{Arc, RwLock};

use serde::Deserialize;
use tokio::sync::mpsc;

use virs_type::WsFeedEvent;

use crate::adapter::binance::fapi;
use crate::adapter::binance::user_data_ws_events::dispatch_event;
use crate::auth::Signer;
use virs_ws::{MessageOutcome, WsHandler, WsManager, WsManagerConfig, WsManagerEvent};
use crate::ExchangeClient;
use virs_task::{spawn, Stop, TaskHandle};

#[derive(Debug, Clone, Deserialize)]
pub struct BinanceOrderMessage {
    pub(crate) data: Option<BinanceOrderData>,

    #[serde(rename = "e")]
    pub(crate) event_type_flat: Option<String>,

    #[serde(rename = "E")]
    event_time_flat: Option<i64>,
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
}

#[derive(Debug, Clone, Deserialize)]
pub struct BinanceOrderData {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: i64,
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

    async fn refresh_url(&self) -> Result<String, virs_error::VirsError> {
        let new_key = fapi::create_listen_key(&self.client, self.signer.as_ref()).await?;
        *self.current_key.write().expect("listenKey RwLock poisoned") = new_key.clone();
        let url = format!("wss://fstream.binance.com/private/ws?listenKey={}", new_key);
        tracing::info!("Refreshed listenKey for reconnect");
        Ok(url)
    }

    async fn on_message(&self, text: &str) -> Result<MessageOutcome<WsFeedEvent>, virs_error::VirsError> {
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
                            "Order event delay exceeds threshold"
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
                        "listenKey expired — requesting reconnect with fresh key"
                    );
                    return Ok(MessageOutcome::Reconnect);
                }
                if et == "serverShutdown" {
                    tracing::warn!("Server shutdown event — requesting reconnect");
                    return Ok(MessageOutcome::Reconnect);
                }
            }
        }

        Ok(MessageOutcome::Continue(vec![]))
    }

    async fn on_connected(&self, _is_reconnect: bool) -> Vec<String> {
        vec![]
    }

    async fn on_disconnected(&self) {}
}

pub struct UserDataWs {
    manager: WsManager<WsFeedEvent>,
    config: WsManagerConfig,
    pub ws_url: String,
    current_key: Arc<RwLock<String>>,
    forward_task: std::sync::Mutex<Option<TaskHandle>>,
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

        Self {
            manager: WsManager::new(handler),
            config: WsManagerConfig::default(),
            ws_url,
            current_key,
            forward_task: std::sync::Mutex::new(None),
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

        self.manager
            .start(self.config.clone(), manager_tx)
            .await;

        let handle = spawn("user_data_forward", move |stop: Stop| async move {
            loop {
                tokio::select! {
                    _ = stop.cancelled() => break,
                    ev = manager_rx.recv() => {
                        let Some(ev) = ev else { break };
                        let feed_event = match ev {
                            WsManagerEvent::Message(e) => e,
                            WsManagerEvent::ConnectionChanged { connected, .. } => {
                                WsFeedEvent::ConnectionChanged { connected }
                            }
                            WsManagerEvent::CircuitBreakerTripped { retry_count } => {
                                tracing::error!(
                                    retry_count = retry_count,
                                    "Circuit breaker tripped — WS stopped after max retries"
                                );
                                WsFeedEvent::ConnectionChanged { connected: false }
                            }
                        };
                        if event_tx.send(feed_event).await.is_err() {
                            tracing::warn!(
                                "External event channel closed, stopping forwarder"
                            );
                            break;
                        }
                    }
                }
            }
        });

        *self.forward_task.lock().unwrap() = Some(handle);
    }

    pub async fn stop(&self) {
        self.manager.stop().await;
        let handle = self.forward_task.lock().unwrap().take();
        if let Some(h) = handle {
            h.cancel();
            h.join().await;
        }
    }
}
