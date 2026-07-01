//! OrderBookEngine — real-time order book streaming engine.
//!
//! Manages WebSocket subscriptions to exchange order book streams,
//! caches the latest snapshot per symbol, and broadcasts updates
//! to all subscribers via a tokio broadcast channel.
//!
//! Architecture mirrors KlineEngine but is simpler:
//! - No aggregation (order book is a snapshot, not a time series)
//! - No gap detection / backfill (each WS message is a complete top-N snapshot)
//! - No persistence (order book state is ephemeral)

use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::{broadcast, Mutex};

use crate::types::{
    subscription_key, MarketType, OrderBookEngineConfig, OrderBookEvent, OrderBookWsClient,
    WsOrderBookEvent,
};

struct SubscriptionEntry {
    exchange: String,
    symbol: String,
    market_type: MarketType,
}

struct MarketWsHandler {
    ws: Arc<Mutex<dyn OrderBookWsClient>>,
}

impl MarketWsHandler {
    fn new(ws: Arc<Mutex<dyn OrderBookWsClient>>) -> Self {
        Self { ws }
    }

    async fn start(&self, update_tx: broadcast::Sender<WsOrderBookEvent>) {
        let mut ws = self.ws.lock().await;
        ws.start(update_tx).await;
    }

    async fn stop(&self) {
        let mut ws = self.ws.lock().await;
        ws.stop().await;
    }

    async fn subscribe(&self, symbol: &str) {
        let ws = self.ws.lock().await;
        ws.subscribe(symbol).await;
    }

    async fn unsubscribe(&self, symbol: &str) {
        let ws = self.ws.lock().await;
        ws.unsubscribe(symbol).await;
    }
}

pub struct OrderBookEngine {
    subscriptions: Arc<DashMap<String, SubscriptionEntry>>,
    /// Reverse index: symbol → subscription key (for fast WS event lookup)
    symbol_index: Arc<DashMap<String, String>>,
    event_tx: broadcast::Sender<OrderBookEvent>,
    spot_handler: MarketWsHandler,
    perpetual_handler: MarketWsHandler,
    started: Arc<std::sync::atomic::AtomicBool>,
}

impl OrderBookEngine {
    pub fn new(
        config: OrderBookEngineConfig,
        spot_ws: Arc<Mutex<dyn OrderBookWsClient>>,
        perpetual_ws: Arc<Mutex<dyn OrderBookWsClient>>,
    ) -> Self {
        let (event_tx, _) = broadcast::channel(config.event_channel_capacity);

        Self {
            subscriptions: Arc::new(DashMap::new()),
            symbol_index: Arc::new(DashMap::new()),
            event_tx,
            spot_handler: MarketWsHandler::new(spot_ws),
            perpetual_handler: MarketWsHandler::new(perpetual_ws),
            started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<OrderBookEvent> {
        self.event_tx.subscribe()
    }

    pub async fn start(&self) {
        if self
            .started
            .swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            return;
        }

        tracing::debug!("[OrderBookEngine] Starting...");

        let (ws_update_tx, mut ws_update_rx) = broadcast::channel::<WsOrderBookEvent>(4096);

        self.spot_handler.start(ws_update_tx.clone()).await;
        self.perpetual_handler.start(ws_update_tx).await;

        let event_tx = self.event_tx.clone();
        let subscriptions = self.subscriptions.clone();
        let symbol_index = self.symbol_index.clone();
        let started = self.started.clone();

        // WS update processor
        tokio::spawn(async move {
            tracing::debug!("[OrderBookEngine] WS update processor started");

            while started.load(std::sync::atomic::Ordering::Relaxed) {
                match ws_update_rx.recv().await {
                    Ok(WsOrderBookEvent::Reconnected) => {
                        tracing::debug!(
                            "[OrderBookEngine] WS reconnected — snapshots will resume automatically"
                        );
                    }
                    Ok(WsOrderBookEvent::OrderBook(update)) => {
                        let symbol = update.symbol;
                        let sub_key = match symbol_index.get(&symbol).map(|r| r.value().clone()) {
                            Some(key) => key,
                            None => continue,
                        };

                        let exchange = match subscriptions.get(&sub_key) {
                            Some(entry) => entry.exchange.clone(),
                            None => continue,
                        };

                        let event = OrderBookEvent {
                            exchange,
                            symbol: symbol.clone(),
                            bids: update.bids,
                            asks: update.asks,
                            timestamp: update.timestamp,
                        };

                        let _ = event_tx.send(event);
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("[OrderBookEngine] WS update lagged by {} messages", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::debug!("[OrderBookEngine] WS update channel closed");
                        break;
                    }
                }
            }

            tracing::debug!("[OrderBookEngine] WS update processor stopped");
        });

        tracing::debug!("[OrderBookEngine] Started successfully");
    }

    pub async fn stop(&self) {
        if !self
            .started
            .swap(false, std::sync::atomic::Ordering::Relaxed)
        {
            return;
        }
        tracing::debug!("[OrderBookEngine] Stopping...");
        self.spot_handler.stop().await;
        self.perpetual_handler.stop().await;
        tracing::debug!("[OrderBookEngine] Stopped");
    }

    pub async fn subscribe(
        &self,
        exchange: &str,
        symbol: &str,
        market_type: MarketType,
    ) -> anyhow::Result<()> {
        // Lazy start: auto-start the engine on first subscription
        if !self.started.load(std::sync::atomic::Ordering::Relaxed) {
            self.start().await;
        }

        let key = subscription_key(exchange, symbol);

        if self.subscriptions.contains_key(&key) {
            tracing::debug!(
                "[OrderBookEngine] Already subscribed to {}/{}",
                exchange,
                symbol
            );
            return Ok(());
        }

        let entry = SubscriptionEntry {
            exchange: exchange.to_string(),
            symbol: symbol.to_string(),
            market_type,
        };

        self.subscriptions.insert(key.clone(), entry);
        self.symbol_index.insert(symbol.to_string(), key);

        match market_type {
            MarketType::Spot => {
                self.spot_handler.subscribe(symbol).await;
            }
            MarketType::Perpetual => {
                self.perpetual_handler.subscribe(symbol).await;
            }
        }

        tracing::debug!(
            "[OrderBookEngine] Subscribed to {}/{} ({})",
            exchange,
            symbol,
            market_type
        );
        Ok(())
    }

    pub async fn unsubscribe(&self, exchange: &str, symbol: &str) -> anyhow::Result<()> {
        let key = subscription_key(exchange, symbol);

        let market_type = match self.subscriptions.get(&key) {
            Some(entry) => entry.market_type,
            None => return Ok(()),
        };

        match market_type {
            MarketType::Spot => {
                self.spot_handler.unsubscribe(symbol).await;
            }
            MarketType::Perpetual => {
                self.perpetual_handler.unsubscribe(symbol).await;
            }
        }

        self.subscriptions.remove(&key);
        self.symbol_index.remove(symbol);

        tracing::debug!(
            "[OrderBookEngine] Unsubscribed from {}/{}",
            exchange,
            symbol
        );
        Ok(())
    }

    pub fn is_subscribed(&self, exchange: &str, symbol: &str) -> bool {
        let key = subscription_key(exchange, symbol);
        self.subscriptions.contains_key(&key)
    }
}
