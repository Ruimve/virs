use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::{broadcast, Mutex};
use virs_error::VirsResult;
use virs_runtime::{CancellationToken, TaskSupervisor};

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
    /// 任务监督器 — 统一管理 JoinHandle + 取消信号 + 优雅关闭
    /// std::sync::Mutex：锁仅在 start/stop/Drop 中短暂持有，不跨 await，Drop 中安全
    supervisor: std::sync::Mutex<Option<TaskSupervisor>>,
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
            supervisor: std::sync::Mutex::new(None),
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

        let cancel = CancellationToken::root();
        let supervisor = TaskSupervisor::new(cancel.clone());

        let (ws_update_tx, mut ws_update_rx) = broadcast::channel::<WsOrderBookEvent>(512);

        self.perpetual_handler.start(ws_update_tx).await;

        let event_tx = self.event_tx.clone();
        let subscriptions = self.subscriptions.clone();
        let symbol_index = self.symbol_index.clone();

        supervisor
            .spawn_raw("orderbook_ws_loop", move |task_cancel| async move {
                loop {
                    tokio::select! {
                        _ = task_cancel.cancelled() => break,
                        result = ws_update_rx.recv() => {
                            match result {
                                Ok(WsOrderBookEvent::Reconnected) => {}
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
                                            tracing::debug!("OrderBookEvent broadcast — receiver dropped between check and send");
                                        }
                                    }
                                }
                                Err(broadcast::error::RecvError::Lagged(n)) => {
                                    tracing::warn!(lagged = n, "WS update lagged");
                                }
                                Err(broadcast::error::RecvError::Closed) => {
                                    break;
                                }
                            }
                        }
                    }
                }
            })
            .await;

        // 存储 TaskSupervisor 供 stop() 调用 shutdown()
        *self.supervisor.lock().unwrap() = Some(supervisor);
    }

    pub async fn stop(&self) {
        if !self
            .started
            .swap(false, std::sync::atomic::Ordering::Relaxed)
        {
            return;
        }
        // 通过 TaskSupervisor::shutdown 统一关闭：cancel + 并发等待 + 超时 abort
        let supervisor = self.supervisor.lock().unwrap().take();
        if let Some(s) = supervisor {
            s.shutdown().await;
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

impl Drop for OrderBookEngine {
    fn drop(&mut self) {
        // RAII 兜底：即使未调用 stop()，也确保取消信号被触发
        if let Some(s) = self.supervisor.lock().unwrap().take() {
            s.cancel().cancel();
        }
    }
}
