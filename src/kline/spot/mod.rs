use std::sync::Arc;

use tokio::sync::{broadcast, Mutex};

use super::types::{KlineWsClient, WsEvent};
use crate::models::MarketType;

pub struct SpotHandler {
    ws: Arc<Mutex<dyn KlineWsClient>>,
}

impl SpotHandler {
    pub fn new(ws: Arc<Mutex<dyn KlineWsClient>>) -> Self {
        Self { ws }
    }

    pub fn market_type(&self) -> MarketType {
        MarketType::Spot
    }

    pub async fn start(&self, update_tx: broadcast::Sender<WsEvent>) {
        let mut ws = self.ws.lock().await;
        ws.start(update_tx).await;
    }

    pub async fn stop(&self) {
        let mut ws = self.ws.lock().await;
        ws.stop().await;
    }

    pub async fn subscribe(&self, symbol: &str) {
        let ws = self.ws.lock().await;
        ws.subscribe(symbol).await;
    }

    pub async fn unsubscribe(&self, symbol: &str) {
        let ws = self.ws.lock().await;
        ws.unsubscribe(symbol).await;
    }

    pub async fn is_running(&self) -> bool {
        let ws = self.ws.lock().await;
        ws.is_running()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    struct MockSpotWs {
        started: AtomicBool,
        sub_count: AtomicUsize,
        unsub_count: AtomicUsize,
    }

    impl MockSpotWs {
        fn new() -> Self {
            Self {
                started: AtomicBool::new(false),
                sub_count: AtomicUsize::new(0),
                unsub_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl KlineWsClient for MockSpotWs {
        async fn start(&mut self, _update_tx: broadcast::Sender<WsEvent>) {
            self.started.store(true, Ordering::Relaxed);
        }
        async fn stop(&mut self) {
            self.started.store(false, Ordering::Relaxed);
        }
        async fn subscribe(&self, _symbol: &str) {
            self.sub_count.fetch_add(1, Ordering::Relaxed);
        }
        async fn unsubscribe(&self, _symbol: &str) {
            self.unsub_count.fetch_add(1, Ordering::Relaxed);
        }
        fn is_running(&self) -> bool {
            self.started.load(Ordering::Relaxed)
        }
    }

    fn make_handler() -> SpotHandler {
        SpotHandler::new(Arc::new(Mutex::new(MockSpotWs::new())))
    }

    #[tokio::test]
    async fn test_spot_handler_market_type() {
        let handler = make_handler();
        assert_eq!(handler.market_type(), MarketType::Spot);
    }

    #[tokio::test]
    async fn test_spot_handler_start_stop() {
        let handler = make_handler();
        let (tx, _) = broadcast::channel(16);
        handler.start(tx).await;
        assert!(handler.is_running().await);
        handler.stop().await;
        assert!(!handler.is_running().await);
    }

    #[tokio::test]
    async fn test_spot_handler_subscribe() {
        let handler = make_handler();
        handler.subscribe("BTCUSDT").await;
        handler.subscribe("ETHUSDT").await;
    }

    #[tokio::test]
    async fn test_spot_handler_unsubscribe() {
        let handler = make_handler();
        handler.subscribe("BTCUSDT").await;
        handler.unsubscribe("BTCUSDT").await;
    }

    #[tokio::test]
    async fn test_spot_handler_not_running_initially() {
        let handler = make_handler();
        assert!(!handler.is_running().await);
    }

    #[tokio::test]
    async fn test_spot_handler_idempotent_start() {
        let handler = make_handler();
        let (tx, _) = broadcast::channel(16);
        handler.start(tx).await;
        assert!(handler.is_running().await);
        let (tx2, _) = broadcast::channel(16);
        handler.start(tx2).await;
        assert!(handler.is_running().await);
    }
}
