use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use tokio::sync::{broadcast, Mutex};
use tracing;
use virs_error::VirsResult;

use crate::aggregator::Aggregator;
use crate::cache::SymbolCache;
use crate::gap::GapDetector;
pub use crate::source::ExchangeKlineSource;
use crate::types::{
    subscription_key, Candle, KlineEngineConfig, KlineEvent, KlineEventType, KlinePersistence,
    KlineSource, KlineWsClient, MarketType, Timeframe, WsEvent,
};

struct NoOpPersistence;

#[async_trait]
impl KlinePersistence for NoOpPersistence {
    async fn save_candles(
        &self,
        _exchange: &str,
        _symbol: &str,
        _timeframe: &str,
        _candles: &[Candle],
    ) -> VirsResult<()> {
        Ok(())
    }
}

struct SubscriptionEntry {
    exchange: String,
    symbol: String,
    market_type: MarketType,
    cache: Arc<Mutex<SymbolCache>>,
}

struct MarketWsHandler {
    ws: Arc<Mutex<dyn KlineWsClient>>,
}

impl MarketWsHandler {
    fn new(ws: Arc<Mutex<dyn KlineWsClient>>) -> Self {
        Self { ws }
    }

    async fn start(&self, update_tx: broadcast::Sender<WsEvent>) {
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

    async fn _is_running(&self) -> bool {
        let ws = self.ws.lock().await;
        ws.is_running()
    }
}

pub struct KlineEngine {
    config: KlineEngineConfig,
    source: Arc<dyn KlineSource>,
    persistence: Arc<dyn KlinePersistence>,
    subscriptions: Arc<DashMap<String, SubscriptionEntry>>,
    symbol_index: Arc<DashMap<String, String>>,
    event_tx: broadcast::Sender<KlineEvent>,
    perpetual_handler: MarketWsHandler,
    started: Arc<std::sync::atomic::AtomicBool>,
}

impl KlineEngine {
    pub fn new(
        config: KlineEngineConfig,
        source: Arc<dyn KlineSource>,
        perpetual_ws: Arc<Mutex<dyn KlineWsClient>>,
    ) -> Self {
        let (event_tx, _) = broadcast::channel(config.event_channel_capacity);

        Self {
            config,
            source,
            persistence: Arc::new(NoOpPersistence),
            subscriptions: Arc::new(DashMap::new()),
            symbol_index: Arc::new(DashMap::new()),
            event_tx,
            perpetual_handler: MarketWsHandler::new(perpetual_ws),
            started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<KlineEvent> {
        self.event_tx.subscribe()
    }

    pub async fn start(&self) {
        if self
            .started
            .swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            return;
        }

        let event_tx = self.event_tx.clone();
        let subscriptions = self.subscriptions.clone();
        let symbol_index = self.symbol_index.clone();
        let source = self.source.clone();
        let persistence = self.persistence.clone();
        let started = self.started.clone();

        let gap_check_subscriptions = subscriptions.clone();
        let gap_check_source = source.clone();
        let gap_check_event_tx = self.event_tx.clone();
        let gap_check_started = started.clone();

        let (ws_update_tx, mut ws_update_rx) = broadcast::channel::<WsEvent>(512);

        self.perpetual_handler.start(ws_update_tx).await;


        tokio::spawn(async move {
            while started.load(std::sync::atomic::Ordering::Relaxed) {
                match ws_update_rx.recv().await {
                    Ok(WsEvent::Reconnected) => {


                        let entries: Vec<_> = subscriptions
                            .iter()
                            .map(|e| {
                                let sub = e.value();
                                (
                                    sub.exchange.clone(),
                                    sub.symbol.clone(),
                                    sub.cache.clone(),
                                    sub.market_type,
                                )
                            })
                            .collect();
                        for (exchange, symbol, cache, market_type) in entries {
                            match GapDetector::detect_and_backfill(
                                &exchange,
                                &symbol,
                                &cache,
                                &source,
                                &event_tx,
                                market_type,
                            )
                            .await
                            {
                                Ok(_) => {}
                                Err(e) => {
                                    tracing::error!("[KlineEngine] Post-reconnect backfill failed for {}/{}: {}", exchange, symbol, e);
                                }
                            }
                        }
                    }
                    Ok(WsEvent::Candle(update)) => {
                        let symbol = update.symbol;
                        let sub_key = match symbol_index.get(&symbol).map(|r| r.value().clone()) {
                            Some(key) => key,
                            None => continue,
                        };

                        let cache = match subscriptions.get(&sub_key) {
                            Some(entry) => entry.cache.clone(),
                            None => continue,
                        };

                        let candle_1m = update.candle;
                        let is_closed = candle_1m.closed;

                        let (exchange, persist_data, higher_updates) = {
                            let mut guard = cache.lock().await;
                            guard.update_candle(Timeframe::M1, candle_1m.clone());
                            if is_closed {
                                guard.close_candle(Timeframe::M1, candle_1m.open_time);
                            }
                            let higher_updates =
                                Aggregator::update_higher_timeframes(&candle_1m, &mut guard);
                            let exchange = match subscriptions.get(&sub_key) {
                                Some(e) => e.exchange.clone(),
                                None => continue,
                            };
                            let persist_data = if is_closed {
                                Some(guard.get_klines(Timeframe::M1))
                            } else {
                                None
                            };
                            (exchange, persist_data, higher_updates)
                        };

                        let event_type = if is_closed {
                            KlineEventType::Closed
                        } else {
                            KlineEventType::Update
                        };

                        if event_tx.receiver_count() > 0 {
                            if event_tx.send(KlineEvent {
                                exchange: exchange.clone(),
                                symbol: symbol.clone(),
                                timeframe: Timeframe::M1,
                                candle: candle_1m.clone(),
                                event_type,
                            }).is_err() {
                                tracing::debug!(
                                    exchange = %exchange,
                                    symbol = %symbol,
                                    "KlineEvent (M1) broadcast — receiver dropped between check and send"
                                );
                            }

                            for (tf, candle) in higher_updates {
                                let ht_event_type = if candle.closed {
                                    KlineEventType::Closed
                                } else {
                                    KlineEventType::Update
                                };
                                if event_tx.send(KlineEvent {
                                    exchange: exchange.clone(),
                                    symbol: symbol.clone(),
                                    timeframe: tf,
                                    candle,
                                    event_type: ht_event_type,
                                }).is_err() {
                                    tracing::debug!(
                                        exchange = %exchange,
                                        symbol = %symbol,
                                        "KlineEvent (higher tf) broadcast — receiver dropped between check and send"
                                    );
                                }
                            }
                        }

                        if let Some(data) = persist_data {
                            if let Err(e) = persistence
                                .save_candles(&exchange, &symbol, "1m", data.as_slice())
                                .await
                            {
                                tracing::warn!(
                                    exchange = %exchange,
                                    symbol = %symbol,
                                    error = %e,
                                    "Failed to save candles"
                                );
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("[KlineEngine] WS update lagged by {} messages", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }

        });


        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));

            while gap_check_started.load(std::sync::atomic::Ordering::Relaxed) {
                interval.tick().await;


                let entries: Vec<_> = gap_check_subscriptions
                    .iter()
                    .map(|e| {
                        let sub = e.value();
                        (
                            sub.exchange.clone(),
                            sub.symbol.clone(),
                            sub.cache.clone(),
                            sub.market_type,
                        )
                    })
                    .collect();
                for (exchange, symbol, cache, market_type) in entries {
                    let report =
                        GapDetector::check_continuity(&exchange, &symbol, &cache).await;

                    if !report.is_continuous {
                        match GapDetector::detect_and_backfill(
                            &exchange,
                            &symbol,
                            &cache,
                            &gap_check_source,
                            &gap_check_event_tx,
                            market_type,
                        )
                        .await
                        {
                            Ok(_) => {}
                            Err(e) => {
                                tracing::error!(
                                    "[KlineEngine] Backfill failed for {}/{}: {}",
                                    exchange,
                                    symbol,
                                    e
                                );
                            }
                        }
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
        market_type: MarketType,
    ) -> VirsResult<()> {

        if !self.started.load(std::sync::atomic::Ordering::Relaxed) {
            self.start().await;
        }

        let key = subscription_key(exchange, symbol);

        if self.subscriptions.contains_key(&key) {
            return Ok(());
        }

        let cache = Arc::new(Mutex::new(SymbolCache::new()));
        let entry = SubscriptionEntry {
            exchange: exchange.to_string(),
            symbol: symbol.to_string(),
            market_type,
            cache: cache.clone(),
        };

        self.subscriptions.insert(key.clone(), entry);
        self.symbol_index.insert(symbol.to_string(), key);

        self.perpetual_handler.subscribe(symbol).await;

        if self.config.backfill_on_start {
            GapDetector::detect_and_backfill(
                exchange,
                symbol,
                &cache,
                &self.source,
                &self.event_tx,
                market_type,
            )
            .await?;
        }

        Ok(())
    }

    pub async fn get_klines_async(
        &self,
        exchange: &str,
        symbol: &str,
        timeframe: Timeframe,
    ) -> Option<Vec<Candle>> {
        let key = subscription_key(exchange, symbol);
        match self.subscriptions.get(&key) {
            Some(entry) => {
                let guard = entry.cache.lock().await;
                Some(guard.get_klines(timeframe))
            }
            None => None,
        }
    }

    pub fn subscribed_symbols(&self) -> Vec<(String, String, MarketType)> {
        self.subscriptions
            .iter()
            .map(|entry| {
                (
                    entry.exchange.clone(),
                    entry.symbol.clone(),
                    entry.market_type,
                )
            })
            .collect()
    }
}
