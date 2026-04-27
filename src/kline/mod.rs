pub mod types;
pub mod cache;
pub mod aggregator;
pub mod gap;

use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use tokio::sync::{broadcast, Mutex};
use tracing;

use self::aggregator::Aggregator;
use self::cache::SymbolCache;
use self::gap::GapDetector;
use self::types::{
    Candle, KlineEngineConfig, KlineEvent, KlineEventType, MarketType, Timeframe,
    AllTimeframesData, BacktestRangeLimit, BacktestRangeInfo, KlineWsClient, KlineSource,
    KlinePersistence, WsEvent, subscription_key,
};

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

    async fn is_running(&self) -> bool {
        let ws = self.ws.lock().await;
        ws.is_running()
    }
}

pub struct KlineEngine {
    config: KlineEngineConfig,
    source: Arc<dyn KlineSource>,
    persistence: Arc<dyn KlinePersistence>,
    subscriptions: Arc<DashMap<String, SubscriptionEntry>>,
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
                                sub.market_type,
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

                        let sub_key = match subscriptions.iter().find(|e| e.value().symbol == symbol) {
                            Some(entry) => subscription_key(&entry.exchange, &symbol),
                            None => continue,
                        };

                        let cache = match subscriptions.get(&sub_key) {
                            Some(entry) => entry.cache.clone(),
                            None => continue,
                        };

                        let candle_1m = update.candle;
                        let is_closed = candle_1m.closed;

                        tracing::info!(
                            "[KlineEngine] WS 1m candle: {} open_time={} close={:.2} volume={:.4} closed={}",
                            symbol, candle_1m.open_time, candle_1m.close, candle_1m.volume, is_closed
                        );

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
                            sub.market_type,
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

        match market_type {
            MarketType::Spot => {
                self.spot_handler.subscribe(symbol).await;
            }
            MarketType::Perpetual => {
                self.perpetual_handler.subscribe(symbol).await;
            }
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
            MarketType::Spot => {
                self.spot_handler.unsubscribe(symbol).await;
            }
            MarketType::Perpetual => {
                self.perpetual_handler.unsubscribe(symbol).await;
            }
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
                (entry.exchange.clone(), entry.symbol.clone(), entry.market_type)
            })
            .collect()
    }

    pub async fn force_backfill(&self, exchange: &str, symbol: &str) -> anyhow::Result<usize> {
        let key = subscription_key(exchange, symbol);
        let (cache, market_type) = match self.subscriptions.get(&key) {
            Some(entry) => (entry.cache.clone(), entry.market_type),
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
        let days = ((end_ms - start_ms) / (86_400_000)) as u32;
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
                    if !filtered.is_empty() {
                        return Ok(filtered);
                    }
                }
            }
        }

        let all_1m = self.source.fetch_klines(
            exchange,
            symbol,
            "1m",
            (days as u32) * 1440 + 100,
            Some(start_ms),
            market_type,
        ).await?;

        if timeframe == Timeframe::M1 {
            return Ok(all_1m);
        }

        let mut cache = SymbolCache::new();
        for candle in &all_1m {
            cache.update_candle(Timeframe::M1, candle.clone());
            if candle.closed {
                cache.close_candle(Timeframe::M1, candle.open_time);
            }
        }

        Ok(cache.get_klines(timeframe)
            .into_iter()
            .filter(|c| c.open_time >= start_ms && c.open_time <= end_ms)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};
    use std::sync::Mutex as StdMutex;
    use std::sync::atomic::{AtomicBool, Ordering};
    use async_trait::async_trait;

    // === Mock KlineWsClient ===

    struct MockKlineWsClient {
        running: Arc<AtomicBool>,
        subscribed_symbols: Arc<StdMutex<HashSet<String>>>,
    }

    impl MockKlineWsClient {
        fn new() -> Self {
            Self {
                running: Arc::new(AtomicBool::new(false)),
                subscribed_symbols: Arc::new(StdMutex::new(HashSet::new())),
            }
        }

        fn subscribed_symbols_ref(&self) -> Arc<StdMutex<HashSet<String>>> {
            self.subscribed_symbols.clone()
        }
    }

    #[async_trait]
    impl KlineWsClient for MockKlineWsClient {
        async fn start(&mut self, _update_tx: broadcast::Sender<WsEvent>) {
            self.running.store(true, Ordering::Relaxed);
        }

        async fn stop(&mut self) {
            self.running.store(false, Ordering::Relaxed);
        }

        async fn subscribe(&self, symbol: &str) {
            self.subscribed_symbols.lock().unwrap().insert(symbol.to_string());
        }

        async fn unsubscribe(&self, symbol: &str) {
            self.subscribed_symbols.lock().unwrap().remove(symbol);
        }

        fn is_running(&self) -> bool {
            self.running.load(Ordering::Relaxed)
        }
    }

    // === Mock KlineSource ===

    struct MockKlineSource {
        data: StdMutex<HashMap<String, Vec<Candle>>>,
    }

    impl MockKlineSource {
        fn new() -> Self {
            Self {
                data: StdMutex::new(HashMap::new()),
            }
        }

        fn add_data(&self, exchange: &str, symbol: &str, timeframe: &str, candles: Vec<Candle>) {
            let key = format!("{}:{}:{}", exchange, symbol, timeframe);
            self.data.lock().unwrap().insert(key, candles);
        }

        fn into_source(self) -> Arc<dyn KlineSource> {
            Arc::new(self)
        }
    }

    #[async_trait]
    impl KlineSource for MockKlineSource {
        async fn fetch_klines(
            &self,
            exchange: &str,
            symbol: &str,
            timeframe: &str,
            _limit: u32,
            since: Option<i64>,
            _market_type: Option<MarketType>,
        ) -> anyhow::Result<Vec<Candle>> {
            let key = format!("{}:{}:{}", exchange, symbol, timeframe);
            let all = self.data.lock().unwrap()
                .get(&key)
                .cloned()
                .unwrap_or_default();
            match since {
                Some(s) => Ok(all.into_iter().filter(|c| c.open_time >= s).collect()),
                None => Ok(all),
            }
        }
    }

    // === Helper functions ===

    fn make_1m_candle(open_time: i64, closed: bool) -> Candle {
        Candle {
            open_time,
            close_time: open_time + 59_999,
            open: 100.0,
            high: 110.0,
            low: 90.0,
            close: 105.0,
            volume: 50.0,
            quote_volume: 5000.0,
            trades: 100,
            closed,
        }
    }

    fn make_1m_sequence(start_time: i64, count: usize, closed: bool) -> Vec<Candle> {
        (0..count)
            .map(|i| make_1m_candle(start_time + i as i64 * 60_000, closed))
            .collect()
    }

    async fn create_test_engine(source: Arc<dyn KlineSource>) -> KlineEngine {
        let spot_ws = Arc::new(Mutex::new(MockKlineWsClient::new()));
        let perpetual_ws = Arc::new(Mutex::new(MockKlineWsClient::new()));
        let config = KlineEngineConfig {
            backfill_on_start: false,
            ..KlineEngineConfig::default()
        };
        KlineEngine::new(config, source, spot_ws, perpetual_ws)
    }

    fn create_test_engine_with_ws(
        source: Arc<dyn KlineSource>,
        spot_ws: MockKlineWsClient,
        perpetual_ws: MockKlineWsClient,
    ) -> (KlineEngine, Arc<StdMutex<HashSet<String>>>, Arc<StdMutex<HashSet<String>>>) {
        let spot_ref = spot_ws.subscribed_symbols_ref();
        let perp_ref = perpetual_ws.subscribed_symbols_ref();
        let spot_ws = Arc::new(Mutex::new(spot_ws));
        let perpetual_ws = Arc::new(Mutex::new(perpetual_ws));
        let config = KlineEngineConfig {
            backfill_on_start: false,
            ..KlineEngineConfig::default()
        };
        let engine = KlineEngine::new(config, source, spot_ws, perpetual_ws);
        (engine, spot_ref, perp_ref)
    }

    // ============================================================
    // Subscription Management (7 tests)
    // ============================================================

    /// Test 1: Subscribing to a symbol creates an entry in the engine.
    #[tokio::test]
    async fn test_subscribe_creates_entry() {
        let source = MockKlineSource::new().into_source();
        let engine = create_test_engine(source).await;

        engine.subscribe("binance", "BTCUSDT", MarketType::Spot).await.unwrap();

        assert!(engine.is_subscribed("binance", "BTCUSDT"));
    }

    /// Test 2: Subscribing to the same symbol twice is idempotent.
    #[tokio::test]
    async fn test_subscribe_idempotent() {
        let source = MockKlineSource::new().into_source();
        let engine = create_test_engine(source).await;

        engine.subscribe("binance", "BTCUSDT", MarketType::Spot).await.unwrap();
        engine.subscribe("binance", "BTCUSDT", MarketType::Spot).await.unwrap();

        assert_eq!(engine.subscribed_symbols().len(), 1);
    }

    /// Test 3: Subscribing to multiple symbols creates separate entries.
    #[tokio::test]
    async fn test_subscribe_multiple_symbols() {
        let source = MockKlineSource::new().into_source();
        let engine = create_test_engine(source).await;

        engine.subscribe("binance", "BTCUSDT", MarketType::Spot).await.unwrap();
        engine.subscribe("binance", "ETHUSDT", MarketType::Spot).await.unwrap();

        let symbols = engine.subscribed_symbols();
        assert_eq!(symbols.len(), 2);
        assert!(engine.is_subscribed("binance", "BTCUSDT"));
        assert!(engine.is_subscribed("binance", "ETHUSDT"));
    }

    /// Test 4: Subscribing with Spot market type calls the spot WS client's subscribe.
    #[tokio::test]
    async fn test_subscribe_spot_calls_spot_ws() {
        let source = MockKlineSource::new().into_source();
        let spot_ws = MockKlineWsClient::new();
        let perpetual_ws = MockKlineWsClient::new();
        let (engine, spot_ref, _perp_ref) = create_test_engine_with_ws(source, spot_ws, perpetual_ws);

        engine.subscribe("binance", "BTCUSDT", MarketType::Spot).await.unwrap();

        let symbols = spot_ref.lock().unwrap();
        assert!(symbols.contains("BTCUSDT"));
    }

    /// Test 5: Subscribing with Perpetual market type calls the perpetual WS client's subscribe.
    #[tokio::test]
    async fn test_subscribe_perpetual_calls_perpetual_ws() {
        let source = MockKlineSource::new().into_source();
        let spot_ws = MockKlineWsClient::new();
        let perpetual_ws = MockKlineWsClient::new();
        let (engine, _spot_ref, perp_ref) = create_test_engine_with_ws(source, spot_ws, perpetual_ws);

        engine.subscribe("binance", "BTCUSDT", MarketType::Perpetual).await.unwrap();

        let symbols = perp_ref.lock().unwrap();
        assert!(symbols.contains("BTCUSDT"));
    }

    /// Test 6: Unsubscribing a previously subscribed symbol removes it.
    #[tokio::test]
    async fn test_unsubscribe() {
        let source = MockKlineSource::new().into_source();
        let engine = create_test_engine(source).await;

        engine.subscribe("binance", "BTCUSDT", MarketType::Spot).await.unwrap();
        assert!(engine.is_subscribed("binance", "BTCUSDT"));

        engine.unsubscribe("binance", "BTCUSDT").await.unwrap();
        assert!(!engine.is_subscribed("binance", "BTCUSDT"));
    }

    /// Test 7: Unsubscribing a symbol that was never subscribed returns Ok without error.
    #[tokio::test]
    async fn test_unsubscribe_nonexistent() {
        let source = MockKlineSource::new().into_source();
        let engine = create_test_engine(source).await;

        let result = engine.unsubscribe("binance", "NONEXISTENT").await;
        assert!(result.is_ok());
    }

    // ============================================================
    // Data Query (4 tests)
    // ============================================================

    /// Test 8: Querying klines for an unsubscribed symbol returns None.
    #[tokio::test]
    async fn test_get_klines_unsubscribed() {
        let source = MockKlineSource::new().into_source();
        let engine = create_test_engine(source).await;

        let result = engine.get_klines("binance", "BTCUSDT", Timeframe::H1);
        assert!(result.is_none());
    }

    /// Test 9: Async querying klines for an unsubscribed symbol returns None.
    #[tokio::test]
    async fn test_get_klines_async_unsubscribed() {
        let source = MockKlineSource::new().into_source();
        let engine = create_test_engine(source).await;

        let result = engine.get_klines_async("binance", "BTCUSDT", Timeframe::H1).await;
        assert!(result.is_none());
    }

    /// Test 10: After subscribing with backfill_on_start=true, get_klines returns data.
    #[tokio::test]
    async fn test_get_klines_after_subscribe_with_backfill() {
        let mock = MockKlineSource::new();
        let candles_1m = make_1m_sequence(0, 10, true);
        mock.add_data("binance", "BTCUSDT", "1m", candles_1m);
        for tf in &["5m", "15m", "1h", "4h", "1d"] {
            mock.add_data("binance", "BTCUSDT", tf, vec![]);
        }
        let source = mock.into_source();

        let spot_ws = Arc::new(Mutex::new(MockKlineWsClient::new()));
        let perpetual_ws = Arc::new(Mutex::new(MockKlineWsClient::new()));
        let config = KlineEngineConfig {
            backfill_on_start: true,
            ..KlineEngineConfig::default()
        };
        let engine = KlineEngine::new(config, source, spot_ws, perpetual_ws);

        engine.subscribe("binance", "BTCUSDT", MarketType::Spot).await.unwrap();

        let result = engine.get_klines_async("binance", "BTCUSDT", Timeframe::M1).await;
        assert!(result.is_some());
        let klines = result.unwrap();
        assert!(klines.len() > 0);
    }

    /// Test 11: After subscribing with backfill, get_all_timeframes returns Some.
    #[tokio::test]
    async fn test_get_all_timeframes() {
        let mock = MockKlineSource::new();
        let candles_1m = make_1m_sequence(0, 10, true);
        mock.add_data("binance", "BTCUSDT", "1m", candles_1m);
        for tf in &["5m", "15m", "1h", "4h", "1d"] {
            mock.add_data("binance", "BTCUSDT", tf, vec![]);
        }
        let source = mock.into_source();

        let spot_ws = Arc::new(Mutex::new(MockKlineWsClient::new()));
        let perpetual_ws = Arc::new(Mutex::new(MockKlineWsClient::new()));
        let config = KlineEngineConfig {
            backfill_on_start: true,
            ..KlineEngineConfig::default()
        };
        let engine = KlineEngine::new(config, source, spot_ws, perpetual_ws);

        engine.subscribe("binance", "BTCUSDT", MarketType::Spot).await.unwrap();

        let result = engine.get_all_timeframes("binance", "BTCUSDT").await;
        assert!(result.is_some());
    }

    // ============================================================
    // Event System (3 tests)
    // ============================================================

    /// Test 12: subscribe_events returns a Receiver without panicking.
    #[tokio::test]
    async fn test_subscribe_events() {
        let source = MockKlineSource::new().into_source();
        let engine = create_test_engine(source).await;

        let _rx = engine.subscribe_events();
        // If we reach here, no panic occurred.
    }

    /// Test 13: Multiple calls to subscribe_events each return a valid Receiver.
    #[tokio::test]
    async fn test_subscribe_events_multiple_receivers() {
        let source = MockKlineSource::new().into_source();
        let engine = create_test_engine(source).await;

        let _rx1 = engine.subscribe_events();
        let _rx2 = engine.subscribe_events();
        // Both receivers created successfully.
    }

    /// Test 14: Subscribing with backfill_on_start=true broadcasts a Backfilled event.
    #[tokio::test]
    async fn test_event_broadcast_on_backfill() {
        let mock = MockKlineSource::new();
        let candles_1m = make_1m_sequence(0, 5, true);
        mock.add_data("binance", "BTCUSDT", "1m", candles_1m);
        for tf in &["5m", "15m", "1h", "4h", "1d"] {
            mock.add_data("binance", "BTCUSDT", tf, vec![]);
        }
        let source = mock.into_source();

        let spot_ws = Arc::new(Mutex::new(MockKlineWsClient::new()));
        let perpetual_ws = Arc::new(Mutex::new(MockKlineWsClient::new()));
        let config = KlineEngineConfig {
            backfill_on_start: true,
            ..KlineEngineConfig::default()
        };
        let engine = KlineEngine::new(config, source, spot_ws, perpetual_ws);

        let mut event_rx = engine.subscribe_events();

        engine.subscribe("binance", "BTCUSDT", MarketType::Spot).await.unwrap();

        let event = event_rx.try_recv();
        assert!(event.is_ok());
        assert_eq!(event.unwrap().event_type, KlineEventType::Backfilled);
    }

    // ============================================================
    // Backtest Tools (4 tests)
    // ============================================================

    /// Test 15: backtest_range_limits returns exactly 6 limit configurations.
    #[tokio::test]
    async fn test_backtest_range_limits() {
        let limits = KlineEngine::backtest_range_limits();
        assert_eq!(limits.len(), 6);
    }

    /// Test 16: validate_backtest_range returns Ok for a valid range.
    #[tokio::test]
    async fn test_validate_backtest_range_valid() {
        let result = KlineEngine::validate_backtest_range(Timeframe::M1, 3);
        assert!(result.is_ok());
    }

    /// Test 17: validate_backtest_range returns Err when days exceed max.
    #[tokio::test]
    async fn test_validate_backtest_range_exceeds_max() {
        let result = KlineEngine::validate_backtest_range(Timeframe::M1, 100);
        assert!(result.is_err());
    }

    /// Test 18: validate_backtest_range returns Ok (with warning) when days exceed recommended but not max.
    #[tokio::test]
    async fn test_validate_backtest_range_recommended_warning() {
        // M1: recommended=3, max=7. Value 5 exceeds recommended but is within max.
        let result = KlineEngine::validate_backtest_range(Timeframe::M1, 5);
        assert!(result.is_ok());
    }

    // ============================================================
    // Continuity / Backfill (2 tests)
    // ============================================================

    /// Test 19: continuity_check returns None for an unsubscribed symbol.
    #[tokio::test]
    async fn test_continuity_check_unsubscribed() {
        let source = MockKlineSource::new().into_source();
        let engine = create_test_engine(source).await;

        let result = engine.continuity_check("binance", "BTCUSDT").await;
        assert!(result.is_none());
    }

    /// Test 20: force_backfill returns Err for an unsubscribed symbol.
    #[tokio::test]
    async fn test_force_backfill_unsubscribed() {
        let source = MockKlineSource::new().into_source();
        let engine = create_test_engine(source).await;

        let result = engine.force_backfill("binance", "BTCUSDT").await;
        assert!(result.is_err());
    }
}
