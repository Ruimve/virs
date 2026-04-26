pub mod types;
pub mod cache;
pub mod aggregator;
pub mod ws;
pub mod gap;
pub mod source;
pub mod api;

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use tokio::sync::{broadcast, Mutex};
use tracing;

use crate::models::MarketType;

use self::aggregator::Aggregator;
use self::cache::SymbolCache;
use self::gap::GapDetector;
use self::types::{
    Candle, KlineEngineConfig, KlineEvent, KlineEventType, Timeframe, AllTimeframesData,
    BacktestRangeLimit, BacktestRangeInfo,
    subscription_key, binance_ws_symbol,
};
use self::ws::{BinanceWs, WsEvent};

#[async_trait]
pub trait KlineSource: Send + Sync {
    async fn fetch_klines(
        &self,
        exchange: &str,
        symbol: &str,
        timeframe: &str,
        limit: u32,
        since: Option<i64>,
        market_type: Option<MarketType>,
    ) -> anyhow::Result<Vec<Candle>>;
}

#[async_trait]
pub trait KlinePersistence: Send + Sync {
    async fn save_candles(
        &self,
        exchange: &str,
        symbol: &str,
        timeframe: &str,
        candles: &[Candle],
    ) -> anyhow::Result<()>;

    async fn load_candles(
        &self,
        exchange: &str,
        symbol: &str,
        timeframe: &str,
    ) -> anyhow::Result<Vec<Candle>>;
}

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

pub struct KlineEngine {
    config: KlineEngineConfig,
    source: Arc<dyn KlineSource>,
    persistence: Arc<dyn KlinePersistence>,
    subscriptions: Arc<DashMap<String, SubscriptionEntry>>,
    ws_symbol_to_key: Arc<DashMap<String, String>>,
    event_tx: broadcast::Sender<KlineEvent>,
    ws_spot: Arc<Mutex<BinanceWs>>,
    ws_perpetual: Arc<Mutex<BinanceWs>>,
    ws_symbol_map: Arc<Mutex<HashMap<String, String>>>,
    started: Arc<std::sync::atomic::AtomicBool>,
}

impl KlineEngine {
    pub fn new(config: KlineEngineConfig, source: Arc<dyn KlineSource>) -> Self {
        let (event_tx, _) = broadcast::channel(config.event_channel_capacity);

        let ws_spot = BinanceWs::new(config.clone(), true);
        let ws_perpetual = BinanceWs::new(config.clone(), false);

        Self {
            config,
            source,
            persistence: Arc::new(NoOpPersistence),
            subscriptions: Arc::new(DashMap::new()),
            ws_symbol_to_key: Arc::new(DashMap::new()),
            event_tx,
            ws_spot: Arc::new(Mutex::new(ws_spot)),
            ws_perpetual: Arc::new(Mutex::new(ws_perpetual)),
            ws_symbol_map: Arc::new(Mutex::new(HashMap::new())),
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
        let ws_spot = self.ws_spot.clone();
        let ws_perpetual = self.ws_perpetual.clone();
        let ws_symbol_map = self.ws_symbol_map.clone();
        let subscriptions = self.subscriptions.clone();
        let ws_symbol_to_key = self.ws_symbol_to_key.clone();
        let source = self.source.clone();
        let persistence = self.persistence.clone();
        let started = self.started.clone();

        let gap_check_subscriptions = subscriptions.clone();
        let gap_check_source = source.clone();
        let gap_check_event_tx = self.event_tx.clone();
        let gap_check_started = started.clone();

        let (ws_update_tx, mut ws_update_rx) = broadcast::channel::<WsEvent>(8192);

        {
            let mut ws = ws_spot.lock().await;
            ws.start(ws_update_tx.clone(), ws_symbol_map.clone()).await;
        }
        {
            let mut ws = ws_perpetual.lock().await;
            ws.start(ws_update_tx, ws_symbol_map.clone()).await;
        }

        tokio::spawn(async move {
            tracing::info!("[KlineEngine] WS update processor started");

            while started.load(std::sync::atomic::Ordering::Relaxed) {
                match ws_update_rx.recv().await {
                    Ok(WsEvent::Reconnected) => {
                        tracing::info!("[KlineEngine] WS reconnected, triggering continuity check for all subscriptions");
                        for entry in subscriptions.iter() {
                            let sub = entry.value();
                            match GapDetector::detect_and_backfill(
                                &sub.exchange,
                                &sub.symbol,
                                &sub.cache,
                                &source,
                                &event_tx,
                                sub.market_type.clone(),
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
                        let ws_sym = update.ws_symbol.to_lowercase();
                        let symbol = {
                            let symbol_map = ws_symbol_map.lock().await;
                            match symbol_map.get(&ws_sym) {
                                Some(s) => s.clone(),
                                None => continue,
                            }
                        };

                        let sub_key = match ws_symbol_to_key.get(&symbol) {
                            Some(k) => k.value().clone(),
                            None => continue,
                        };

                        let cache = match subscriptions.get(&sub_key) {
                            Some(entry) => entry.cache.clone(),
                            None => continue,
                        };

                        let candle_1m = update.candle;
                        let is_closed = candle_1m.closed;

                        let (exchange, persist_data) = {
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

                            let event_type = if is_closed {
                                KlineEventType::Closed
                            } else {
                                KlineEventType::Update
                            };

                            let _ = event_tx.send(KlineEvent {
                                exchange: exchange.clone(),
                                symbol: symbol.clone(),
                                timeframe: Timeframe::M1,
                                candle: candle_1m.clone(),
                                event_type,
                            });

                            for (tf, candle) in higher_updates {
                                let ht_event_type = if candle.closed {
                                    KlineEventType::Closed
                                } else {
                                    KlineEventType::Update
                                };
                                let _ = event_tx.send(KlineEvent {
                                    exchange: exchange.clone(),
                                    symbol: symbol.clone(),
                                    timeframe: tf,
                                    candle,
                                    event_type: ht_event_type,
                                });
                            }

                            let persist_data = if is_closed {
                                Some(guard.get_klines(Timeframe::M1))
                            } else {
                                None
                            };

                            (exchange, persist_data)
                        };

                        if let Some(data) = persist_data {
                            let _ = persistence.save_candles(
                                &exchange,
                                &symbol,
                                "1m",
                                data.as_slice(),
                            ).await;
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

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
            tracing::info!("[KlineEngine] Gap checker started (60s interval)");

            while gap_check_started.load(std::sync::atomic::Ordering::Relaxed) {
                interval.tick().await;

                for entry in gap_check_subscriptions.iter() {
                    let sub = entry.value();

                    let report = GapDetector::check_continuity(
                        &sub.exchange,
                        &sub.symbol,
                        &sub.cache,
                    ).await;

                    if !report.is_continuous {
                        tracing::info!(
                            "[KlineEngine] Gap detected for {}/{}: {} minutes",
                            sub.exchange, sub.symbol, report.missing_minutes
                        );

                        match GapDetector::detect_and_backfill(
                            &sub.exchange,
                            &sub.symbol,
                            &sub.cache,
                            &gap_check_source,
                            &gap_check_event_tx,
                            sub.market_type.clone(),
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

        {
            let mut ws = self.ws_spot.lock().await;
            ws.stop().await;
        }
        {
            let mut ws = self.ws_perpetual.lock().await;
            ws.stop().await;
        }

        tracing::info!("[KlineEngine] Stopped");
    }

    pub async fn subscribe(
        &self,
        exchange: &str,
        symbol: &str,
        market_type: MarketType,
    ) -> anyhow::Result<()> {
        let key = subscription_key(exchange, symbol);

        if self.subscriptions.contains_key(&key) {
            tracing::info!("[KlineEngine] Already subscribed to {}/{}", exchange, symbol);
            return Ok(());
        }

        let cache = Arc::new(Mutex::new(SymbolCache::new()));

        let entry = SubscriptionEntry {
            exchange: exchange.to_string(),
            symbol: symbol.to_string(),
            market_type: market_type.clone(),
            cache: cache.clone(),
        };

        self.subscriptions.insert(key.clone(), entry);
        self.ws_symbol_to_key.insert(symbol.to_string(), key.clone());

        let ws_sym = binance_ws_symbol(symbol);
        {
            let mut map = self.ws_symbol_map.lock().await;
            map.insert(ws_sym.clone(), symbol.to_string());
        }

        let is_spot = market_type == MarketType::Spot;
        let ws = if is_spot {
            self.ws_spot.clone()
        } else {
            self.ws_perpetual.clone()
        };

        {
            let ws_guard = ws.lock().await;
            ws_guard.add_subscription(symbol).await;
        }

        if self.config.backfill_on_start {
            match GapDetector::detect_and_backfill(
                exchange,
                symbol,
                &cache,
                &self.source,
                &self.event_tx,
                market_type,
            ).await {
                Ok(count) => {
                    tracing::info!("[KlineEngine] Initial load: {} candles for {}/{}", count, exchange, symbol);
                }
                Err(e) => {
                    tracing::error!("[KlineEngine] Initial load failed for {}/{}: {}", exchange, symbol, e);
                }
            }
        }

        tracing::info!("[KlineEngine] Subscribed to {}/{} ({})", exchange, symbol, if is_spot { "spot" } else { "perpetual" });
        Ok(())
    }

    pub async fn unsubscribe(&self, exchange: &str, symbol: &str) -> anyhow::Result<()> {
        let key = subscription_key(exchange, symbol);

        let (market_type, cache) = match self.subscriptions.get(&key) {
            Some(entry) => (entry.market_type.clone(), entry.cache.clone()),
            None => return Ok(()),
        };

        let ws_sym = binance_ws_symbol(symbol);
        {
            let mut map = self.ws_symbol_map.lock().await;
            map.remove(&ws_sym);
        }

        let is_spot = market_type == MarketType::Spot;
        let ws = if is_spot {
            self.ws_spot.clone()
        } else {
            self.ws_perpetual.clone()
        };

        {
            let ws_guard = ws.lock().await;
            ws_guard.remove_subscription(symbol).await;
        }

        let persist_data: Vec<(Timeframe, Vec<Candle>)> = {
            let guard = cache.lock().await;
            Timeframe::all().iter().filter_map(|&tf| {
                let candles = guard.get_klines(tf);
                if candles.is_empty() {
                    None
                } else {
                    Some((tf, candles))
                }
            }).collect()
        };

        for (tf, candles) in &persist_data {
            let _ = self.persistence.save_candles(exchange, symbol, tf.as_str(), candles).await;
        }

        self.subscriptions.remove(&key);
        self.ws_symbol_to_key.remove(symbol);

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
        self.subscriptions
            .iter()
            .map(|entry| {
                (entry.exchange.clone(), entry.symbol.clone(), entry.market_type.clone())
            })
            .collect()
    }

    pub async fn force_backfill(&self, exchange: &str, symbol: &str) -> anyhow::Result<usize> {
        let key = subscription_key(exchange, symbol);
        let (cache, market_type) = match self.subscriptions.get(&key) {
            Some(entry) => (entry.cache.clone(), entry.market_type.clone()),
            None => return Err(anyhow::anyhow!("Not subscribed to {}/{}", exchange, symbol)),
        };

        GapDetector::detect_and_backfill(exchange, symbol, &cache, &self.source, &self.event_tx, market_type).await
    }

    pub async fn continuity_check(&self, exchange: &str, symbol: &str) -> Option<gap::ContinuityReport> {
        let key = subscription_key(exchange, symbol);
        match self.subscriptions.get(&key) {
            Some(entry) => {
                Some(GapDetector::check_continuity(exchange, symbol, &entry.cache).await)
            }
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
                "Backtest range {} days exceeds maximum {} days for {} timeframe. Estimated 1m candles required: {} ({} MB). Use direct fetch mode for longer ranges.",
                days, limit.max_days, timeframe, limit.estimated_1m_required,
                limit.estimated_1m_required * 80 / 1024 / 1024
            ));
        }
        if days > limit.recommended_days {
            tracing::warn!(
                "[KlineEngine] Backtest range {} days exceeds recommended {} days for {} timeframe. Performance may degrade.",
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
        let days = ((end_ms - start_ms) / (86_400_000)) as u32;
        Self::validate_backtest_range(timeframe, days)?;

        let market_type = self.subscriptions.get(&subscription_key(exchange, symbol))
            .map(|e| e.market_type.clone());

        let cache_data = self.get_klines_async(exchange, symbol, timeframe).await;
        let cache_start = cache_data.as_ref().and_then(|c| c.first().map(|f| f.open_time));
        let cache_end = cache_data.as_ref().and_then(|c| c.last().map(|l| l.open_time));

        match (cache_start, cache_end) {
            (Some(cs), Some(ce)) if cs <= start_ms && ce >= end_ms => {
                let filtered: Vec<Candle> = cache_data
                    .unwrap()
                    .into_iter()
                    .filter(|c| c.open_time >= start_ms && c.open_time <= end_ms)
                    .collect();
                if !filtered.is_empty() {
                    return Ok(filtered);
                }
            }
            (Some(cs), Some(ce)) if cs <= end_ms && ce >= start_ms => {
                let mut result: Vec<Candle> = Vec::new();

                if cs > start_ms {
                    let limit = Self::calculate_fetch_limit(timeframe, start_ms, cs);
                    if let Ok(pre) = self.source.fetch_klines(exchange, symbol, timeframe.as_str(), limit, Some(start_ms), market_type.clone()).await {
                        result.extend(pre.into_iter().filter(|c| c.open_time < cs));
                    }
                }

                if let Some(cached) = cache_data {
                    result.extend(cached.into_iter().filter(|c| c.open_time >= start_ms && c.open_time <= end_ms));
                }

                if ce < end_ms {
                    let limit = Self::calculate_fetch_limit(timeframe, ce, end_ms);
                    if let Ok(post) = self.source.fetch_klines(exchange, symbol, timeframe.as_str(), limit, Some(ce), market_type.clone()).await {
                        result.extend(post.into_iter().filter(|c| c.open_time > ce && c.open_time <= end_ms));
                    }
                }

                if !result.is_empty() {
                    result.sort_by_key(|c| c.open_time);
                    return Ok(result);
                }
            }
            _ => {}
        }

        let limit = Self::calculate_fetch_limit(timeframe, start_ms, end_ms);
        self.source.fetch_klines(exchange, symbol, timeframe.as_str(), limit, Some(start_ms), market_type).await
    }

    fn calculate_fetch_limit(tf: Timeframe, start_ms: i64, end_ms: i64) -> u32 {
        let bars = ((end_ms - start_ms) / tf.ms()) as u32;
        bars.max(1).min(5000)
    }

    pub async fn fetch_backtest_data_multi_tf(
        &self,
        exchange: &str,
        symbol: &str,
        timeframes: &[Timeframe],
        start_ms: i64,
        end_ms: i64,
    ) -> anyhow::Result<std::collections::HashMap<Timeframe, Vec<Candle>>> {
        let mut result = std::collections::HashMap::new();

        for &tf in timeframes {
            let data = self.fetch_backtest_data(exchange, symbol, tf, start_ms, end_ms).await?;
            result.insert(tf, data);
        }

        Ok(result)
    }
}
