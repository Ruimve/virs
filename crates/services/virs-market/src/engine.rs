//! KlineEngine — market data collection and aggregation engine.
//!
//! Manages WebSocket subscriptions, candle aggregation, gap detection,
//! and provides real-time kline data for all timeframes.

use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use tokio::sync::{broadcast, Mutex};
use tracing;

use crate::aggregator::Aggregator;
use crate::cache::SymbolCache;
use crate::gap::GapDetector;
use crate::types::{
    Candle, KlineEngineConfig, KlineEvent, KlineEventType, MarketType, Timeframe,
    AllTimeframesData, BacktestRangeInfo, BacktestRangeLimit, KlineWsClient, KlineSource,
    KlinePersistence, WsEvent, subscription_key,
};
pub use crate::source::ExchangeKlineSource;

struct NoOpPersistence;

#[async_trait]
impl KlinePersistence for NoOpPersistence {
    async fn save_candles(&self, _exchange: &str, _symbol: &str, _timeframe: &str, _candles: &[Candle]) -> anyhow::Result<()> {
        Ok(())
    }
    async fn load_candles(&self, _exchange: &str, _symbol: &str, _timeframe: &str) -> anyhow::Result<Vec<Candle>> {
        Ok(Vec::new())
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

    async fn unsubscribe(&self, symbol: &str) {
        let ws = self.ws.lock().await;
        ws.unsubscribe(symbol).await;
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
    spot_handler: MarketWsHandler,
    perpetual_handler: MarketWsHandler,
    started: Arc<std::sync::atomic::AtomicBool>,
}

impl KlineEngine {
    pub fn new(
        config: KlineEngineConfig,
        source: Arc<dyn KlineSource>,
        spot_ws: Arc<Mutex<dyn KlineWsClient>>,
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
            spot_handler: MarketWsHandler::new(spot_ws),
            perpetual_handler: MarketWsHandler::new(perpetual_ws),
            started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub fn with_persistence(mut self, persistence: Arc<dyn KlinePersistence>) -> Self {
        self.persistence = persistence;
        self
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<KlineEvent> {
        self.event_tx.subscribe()
    }

    pub async fn start(&self) {
        if self.started.swap(true, std::sync::atomic::Ordering::Relaxed) {
            return;
        }

        tracing::info!("[KlineEngine] Starting...");

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

        let (ws_update_tx, mut ws_update_rx) = broadcast::channel::<WsEvent>(8192);

        self.spot_handler.start(ws_update_tx.clone()).await;
        self.perpetual_handler.start(ws_update_tx).await;

        // WS update processor
        tokio::spawn(async move {
            tracing::info!("[KlineEngine] WS update processor started");

            while started.load(std::sync::atomic::Ordering::Relaxed) {
                match ws_update_rx.recv().await {
                    Ok(WsEvent::Reconnected) => {
                        tracing::info!("[KlineEngine] WS reconnected, triggering continuity check");
                        for entry in subscriptions.iter() {
                            let sub = entry.value();
                            match GapDetector::detect_and_backfill(
                                &sub.exchange, &sub.symbol, &sub.cache, &source, &event_tx, sub.market_type,
                            ).await {
                                Ok(count) if count > 0 => {
                                    tracing::info!("[KlineEngine] Post-reconnect backfill: {} candles for {}/{}", count, sub.exchange, sub.symbol);
                                }
                                Ok(_) => {}
                                Err(e) => {
                                    tracing::error!("[KlineEngine] Post-reconnect backfill failed for {}/{}: {}", sub.exchange, sub.symbol, e);
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

                        tracing::debug!(
                            "[KlineEngine] WS 1m candle: {} open_time={} close={:.2} closed={}",
                            symbol, candle_1m.open_time, candle_1m.close, is_closed
                        );

                        let (exchange, persist_data, higher_updates) = {
                            let mut guard = cache.lock().await;
                            guard.update_candle(Timeframe::M1, candle_1m.clone());
                            if is_closed {
                                guard.close_candle(Timeframe::M1, candle_1m.open_time);
                            }
                            let higher_updates = Aggregator::update_higher_timeframes(&candle_1m, &mut guard);
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

                        let event_type = if is_closed { KlineEventType::Closed } else { KlineEventType::Update };

                        let _ = event_tx.send(KlineEvent {
                            exchange: exchange.clone(),
                            symbol: symbol.clone(),
                            timeframe: Timeframe::M1,
                            candle: candle_1m.clone(),
                            event_type,
                        });

                        for (tf, candle) in higher_updates {
                            let ht_event_type = if candle.closed { KlineEventType::Closed } else { KlineEventType::Update };
                            let _ = event_tx.send(KlineEvent {
                                exchange: exchange.clone(),
                                symbol: symbol.clone(),
                                timeframe: tf,
                                candle,
                                event_type: ht_event_type,
                            });
                        }

                        if let Some(data) = persist_data {
                            let _ = persistence.save_candles(&exchange, &symbol, "1m", data.as_slice()).await;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("[KlineEngine] WS update lagged by {} messages", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!("[KlineEngine] WS update channel closed");
                        break;
                    }
                }
            }

            tracing::info!("[KlineEngine] WS update processor stopped");
        });

        // Gap checker
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
            tracing::info!("[KlineEngine] Gap checker started (60s interval)");

            while gap_check_started.load(std::sync::atomic::Ordering::Relaxed) {
                interval.tick().await;

                for entry in gap_check_subscriptions.iter() {
                    let sub = entry.value();
                    let report = GapDetector::check_continuity(&sub.exchange, &sub.symbol, &sub.cache).await;

                    if !report.is_continuous {
                        tracing::info!("[KlineEngine] Gap detected for {}/{}: {} minutes", sub.exchange, sub.symbol, report.missing_minutes);
                        match GapDetector::detect_and_backfill(
                            &sub.exchange, &sub.symbol, &sub.cache, &gap_check_source, &gap_check_event_tx, sub.market_type,
                        ).await {
                            Ok(count) => {
                                tracing::info!("[KlineEngine] Backfilled {} candles for {}/{}", count, sub.exchange, sub.symbol);
                            }
                            Err(e) => {
                                tracing::error!("[KlineEngine] Backfill failed for {}/{}: {}", sub.exchange, sub.symbol, e);
                            }
                        }
                    }
                }
            }

            tracing::info!("[KlineEngine] Gap checker stopped");
        });

        tracing::info!("[KlineEngine] Started successfully");
    }

    pub async fn stop(&self) {
        if !self.started.swap(false, std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        tracing::info!("[KlineEngine] Stopping...");
        self.spot_handler.stop().await;
        self.perpetual_handler.stop().await;
        tracing::info!("[KlineEngine] Stopped");
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
            tracing::info!("[KlineEngine] Already subscribed to {}/{}", exchange, symbol);
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

        match market_type {
            MarketType::Spot => { self.spot_handler.subscribe(symbol).await; }
            MarketType::Perpetual => { self.perpetual_handler.subscribe(symbol).await; }
        }

        if self.config.backfill_on_start {
            GapDetector::detect_and_backfill(
                exchange, symbol, &cache, &self.source, &self.event_tx, market_type,
            ).await?;
        }

        tracing::info!("[KlineEngine] Subscribed to {}/{} ({})", exchange, symbol, market_type);
        Ok(())
    }

    pub async fn unsubscribe(&self, exchange: &str, symbol: &str) -> anyhow::Result<()> {
        let key = subscription_key(exchange, symbol);

        let (market_type, cache) = match self.subscriptions.get(&key) {
            Some(entry) => (entry.market_type, entry.cache.clone()),
            None => return Ok(()),
        };

        match market_type {
            MarketType::Spot => { self.spot_handler.unsubscribe(symbol).await; }
            MarketType::Perpetual => { self.perpetual_handler.unsubscribe(symbol).await; }
        }

        let persist_data: Vec<(Timeframe, Vec<Candle>)> = {
            let guard = cache.lock().await;
            Timeframe::all().iter().filter_map(|&tf| {
                let candles = guard.get_klines(tf);
                if candles.is_empty() { None } else { Some((tf, candles)) }
            }).collect()
        };

        for (tf, candles) in &persist_data {
            let _ = self.persistence.save_candles(exchange, symbol, tf.as_str(), candles).await;
        }

        self.subscriptions.remove(&key);
        self.symbol_index.remove(symbol);

        tracing::info!("[KlineEngine] Unsubscribed from {}/{}", exchange, symbol);
        Ok(())
    }

    pub fn get_klines(&self, exchange: &str, symbol: &str, timeframe: Timeframe) -> Option<Vec<Candle>> {
        let key = subscription_key(exchange, symbol);
        self.subscriptions.get(&key).map(|entry| entry.cache.blocking_lock().get_klines(timeframe))
    }

    pub async fn get_klines_async(&self, exchange: &str, symbol: &str, timeframe: Timeframe) -> Option<Vec<Candle>> {
        let key = subscription_key(exchange, symbol);
        match self.subscriptions.get(&key) {
            Some(entry) => {
                let guard = entry.cache.lock().await;
                Some(guard.get_klines(timeframe))
            }
            None => None,
        }
    }

    pub async fn get_all_timeframes(&self, exchange: &str, symbol: &str) -> Option<AllTimeframesData> {
        let key = subscription_key(exchange, symbol);
        match self.subscriptions.get(&key) {
            Some(entry) => {
                let guard = entry.cache.lock().await;
                Some(guard.get_all_timeframes())
            }
            None => None,
        }
    }

    pub fn is_subscribed(&self, exchange: &str, symbol: &str) -> bool {
        let key = subscription_key(exchange, symbol);
        self.subscriptions.contains_key(&key)
    }

    pub fn subscribed_symbols(&self) -> Vec<(String, String, MarketType)> {
        self.subscriptions.iter().map(|entry| {
            (entry.exchange.clone(), entry.symbol.clone(), entry.market_type)
        }).collect()
    }

    pub async fn force_backfill(&self, exchange: &str, symbol: &str) -> anyhow::Result<usize> {
        let key = subscription_key(exchange, symbol);
        let (cache, market_type) = match self.subscriptions.get(&key) {
            Some(entry) => (entry.cache.clone(), entry.market_type),
            None => return Err(anyhow::anyhow!("Not subscribed to {}/{}", exchange, symbol)),
        };
        GapDetector::detect_and_backfill(exchange, symbol, &cache, &self.source, &self.event_tx, market_type).await
    }

    pub async fn continuity_check(&self, exchange: &str, symbol: &str) -> Option<crate::gap::ContinuityReport> {
        let key = subscription_key(exchange, symbol);
        match self.subscriptions.get(&key) {
            Some(entry) => Some(GapDetector::check_continuity(exchange, symbol, &entry.cache).await),
            None => None,
        }
    }

    pub fn backtest_range_limits() -> Vec<BacktestRangeInfo> {
        BacktestRangeLimit::all_limits().into_iter().map(|l| l.into()).collect()
    }

    pub fn validate_backtest_range(timeframe: Timeframe, days: u32) -> Result<(), anyhow::Error> {
        let limit = BacktestRangeLimit::for_timeframe(timeframe);
        if days > limit.max_days {
            return Err(anyhow::anyhow!(
                "Backtest range {} days exceeds maximum {} days for {} timeframe",
                days, limit.max_days, timeframe
            ));
        }
        if days > limit.recommended_days {
            tracing::warn!(
                "[KlineEngine] Backtest range {} days exceeds recommended {} days for {} timeframe",
                days, limit.recommended_days, timeframe
            );
        }
        Ok(())
    }

    pub async fn fetch_backtest_data(
        &self,
        exchange: &str,
        symbol: &str,
        timeframe: Timeframe,
        start_ms: i64,
        end_ms: i64,
    ) -> anyhow::Result<Vec<Candle>> {
        let days = ((end_ms - start_ms) / 86_400_000) as u32;
        Self::validate_backtest_range(timeframe, days)?;

        let market_type = self.subscriptions.get(&subscription_key(exchange, symbol))
            .map(|e| e.market_type);

        let cache_data = self.get_klines_async(exchange, symbol, timeframe).await;
        let cache_start = cache_data.as_ref().and_then(|c| c.first().map(|f| f.open_time));
        let cache_end = cache_data.as_ref().and_then(|c| c.last().map(|f| f.open_time));

        if let (Some(cs), Some(ce)) = (cache_start, cache_end) {
            if cs <= start_ms && ce >= end_ms {
                if let Some(candles) = cache_data {
                    let filtered: Vec<Candle> = candles.into_iter()
                        .filter(|c| c.open_time >= start_ms && c.open_time <= end_ms)
                        .collect();
                    if !filtered.is_empty() { return Ok(filtered); }
                }
            }
        }

        let all_1m = self.source.fetch_klines(
            exchange, symbol, "1m",
            ((days as u32) * 1440 + 100).min(1000),
            Some(start_ms),
            market_type,
        ).await?;

        if timeframe == Timeframe::M1 { return Ok(all_1m); }

        let aggregated = Aggregator::aggregate_1m_to_timeframe(&all_1m, timeframe);
        Ok(aggregated.into_iter()
            .filter(|c| c.open_time >= start_ms && c.open_time <= end_ms)
            .collect())
    }
}
