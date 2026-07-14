use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::{broadcast, Mutex};
use virs_error::VirsResult;

use crate::types::{
    subscription_key, MarketType, OrderBookEngineConfig, OrderBookEvent, OrderBookWsClient,
    WsOrderBookEvent,
};

struct SubscriptionEntry {
    exchange: String,
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

}

pub struct OrderBookEngine {
    subscriptions: Arc<DashMap<String, SubscriptionEntry>>,

    symbol_index: Arc<DashMap<String, String>>,
    event_tx: broadcast::Sender<OrderBookEvent>,
    perpetual_handler: MarketWsHandler,
    started: Arc<std::sync::atomic::AtomicBool>,
}

impl OrderBookEngine {
    pub fn new(
        config: OrderBookEngineConfig,
        perpetual_ws: Arc<Mutex<dyn OrderBookWsClient>>,
    ) -> Self {
        let (event_tx, _) = broadcast::channel(config.event_channel_capacity);

        Self {
            subscriptions: Arc::new(DashMap::new()),
            symbol_index: Arc::new(DashMap::new()),
            event_tx,
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

        let (ws_update_tx, mut ws_update_rx) = broadcast::channel::<WsOrderBookEvent>(512);

        self.perpetual_handler.start(ws_update_tx).await;

        let event_tx = self.event_tx.clone();
        let subscriptions = self.subscriptions.clone();
        let symbol_index = self.symbol_index.clone();
        let started = self.started.clone();


        tokio::spawn(async move {
            while started.load(std::sync::atomic::Ordering::Relaxed) {
                match ws_update_rx.recv().await {
                    Ok(WsOrderBookEvent::Reconnected) => {
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

                        if event_tx.receiver_count() > 0 {
                            if event_tx.send(event).is_err() {
                                tracing::debug!("[OrderBookEngine] OrderBookEvent broadcast — receiver dropped between check and send");
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("[OrderBookEngine] WS update lagged by {} messages", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }

        });
    }

    pub async fn stop(&self) {
        if !self
            .started
            .swap(false, std::sync::atomic::Ordering::Relaxed)
        {
            return;
        }
        self.perpetual_handler.stop().await;
    }

    pub async fn subscribe(
        &self,
        exchange: &str,
        symbol: &str,
        _market_type: MarketType,
    ) -> VirsResult<()> {

        if !self.started.load(std::sync::atomic::Ordering::Relaxed) {
            self.start().await;
        }

        let key = subscription_key(exchange, symbol);

        if self.subscriptions.contains_key(&key) {
            return Ok(());
        }

        let entry = SubscriptionEntry {
            exchange: exchange.to_string(),
        };

        self.subscriptions.insert(key.clone(), entry);
        self.symbol_index.insert(symbol.to_string(), key);

        self.perpetual_handler.subscribe(symbol).await;

        Ok(())
    }

}
