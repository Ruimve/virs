use crate::kline::*;
use crate::kline::gap::GapDetector;
use crate::kline::cache::SymbolCache;
use std::collections::HashMap;
use std::sync::Mutex as StdMutex;
use async_trait::async_trait;

struct MockKlineSource {
    data: StdMutex<HashMap<String, Vec<Candle>>>,
    errors: StdMutex<HashMap<String, String>>,
}

impl MockKlineSource {
    fn new() -> Self {
        Self {
            data: StdMutex::new(HashMap::new()),
            errors: StdMutex::new(HashMap::new()),
        }
    }

    fn add_data(&self, exchange: &str, symbol: &str, timeframe: &str, candles: Vec<Candle>) {
        let key = format!("{}:{}:{}", exchange, symbol, timeframe);
        self.data.lock().unwrap().insert(key, candles);
    }

    fn set_error(&self, exchange: &str, symbol: &str, timeframe: &str, error: &str) {
        let key = format!("{}:{}:{}", exchange, symbol, timeframe);
        self.errors.lock().unwrap().insert(key, error.to_string());
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
        if let Some(err) = self.errors.lock().unwrap().get(&key) {
            return Err(anyhow::anyhow!("{}", err));
        }
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

fn make_high_tf_candle(open_time: i64, tf: Timeframe, closed: bool) -> Candle {
    Candle {
        open_time,
        close_time: open_time + tf.ms() - 1,
        open: 100.0,
        high: 110.0,
        low: 90.0,
        close: 105.0,
        volume: 500.0,
        quote_volume: 50000.0,
        trades: 1000,
        closed,
    }
}

#[tokio::test]
async fn test_initial_load_basic() {
    let mock = MockKlineSource::new();
    let now_ms = chrono::Utc::now().timestamp_millis();
    let current_1m_open = (now_ms / 60_000) * 60_000;
    let start_1m = current_1m_open - 2000 * 60_000;

    let candles_1m: Vec<Candle> = (0..2000)
        .map(|i| make_1m_candle(start_1m + i as i64 * 60_000, true))
        .collect();
    mock.add_data("binance", "BTCUSDT", "1m", candles_1m.clone());

    for tf_str in &["5m", "15m", "1h", "4h", "1d"] {
        let tf = Timeframe::from_str_lossy(tf_str).unwrap();
        let high_candles: Vec<Candle> = (0..1000)
            .map(|i| make_high_tf_candle(current_1m_open - (i as i64 + 1) * tf.ms(), tf, true))
            .collect();
        mock.add_data("binance", "BTCUSDT", tf_str, high_candles);
    }

    let source = mock.into_source();
    let cache = Arc::new(Mutex::new(SymbolCache::new()));
    let (event_tx, _) = tokio::sync::broadcast::channel(100);

    let result = GapDetector::detect_and_backfill(
        "binance", "BTCUSDT", &cache, &source, &event_tx, MarketType::Spot,
    ).await.unwrap();

    assert!(result > 0);
    let guard = cache.lock().await;
    assert!(guard.candle_count(Timeframe::M1) > 0);
}

#[tokio::test]
async fn test_initial_load_no_1m_data() {
    let source = MockKlineSource::new().into_source();
    let cache = Arc::new(Mutex::new(SymbolCache::new()));
    let (event_tx, _) = tokio::sync::broadcast::channel(100);

    let result = GapDetector::detect_and_backfill(
        "binance", "BTCUSDT", &cache, &source, &event_tx, MarketType::Spot,
    ).await.unwrap();

    assert_eq!(result, 0);
}

#[tokio::test]
async fn test_initial_load_1m_error() {
    let mock = MockKlineSource::new();
    mock.set_error("binance", "BTCUSDT", "1m", "network error");
    let source = mock.into_source();

    let cache = Arc::new(Mutex::new(SymbolCache::new()));
    let (event_tx, _) = tokio::sync::broadcast::channel(100);

    let result = GapDetector::detect_and_backfill(
        "binance", "BTCUSDT", &cache, &source, &event_tx, MarketType::Spot,
    ).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_initial_load_high_tf_partial_failure() {
    let mock = MockKlineSource::new();
    let now_ms = chrono::Utc::now().timestamp_millis();
    let current_1m_open = (now_ms / 60_000) * 60_000;
    let start_1m = current_1m_open - 2000 * 60_000;

    let candles_1m: Vec<Candle> = (0..2000)
        .map(|i| make_1m_candle(start_1m + i as i64 * 60_000, true))
        .collect();
    mock.add_data("binance", "BTCUSDT", "1m", candles_1m);

    mock.add_data("binance", "BTCUSDT", "5m", make_1m_sequence(0, 10, true));
    mock.set_error("binance", "BTCUSDT", "15m", "timeout");
    mock.add_data("binance", "BTCUSDT", "1h", make_1m_sequence(0, 10, true));
    mock.set_error("binance", "BTCUSDT", "4h", "timeout");
    mock.add_data("binance", "BTCUSDT", "1d", make_1m_sequence(0, 10, true));

    let source = mock.into_source();
    let cache = Arc::new(Mutex::new(SymbolCache::new()));
    let (event_tx, _) = tokio::sync::broadcast::channel(100);

    let result = GapDetector::detect_and_backfill(
        "binance", "BTCUSDT", &cache, &source, &event_tx, MarketType::Spot,
    ).await.unwrap();

    assert!(result > 0);
}

#[tokio::test]
async fn test_no_gap_when_up_to_date() {
    let source = MockKlineSource::new().into_source();
    let cache = Arc::new(Mutex::new(SymbolCache::new()));

    let now_ms = chrono::Utc::now().timestamp_millis();
    let current_1m_open = (now_ms / 60_000) * 60_000;
    let recent_candle = make_1m_candle(current_1m_open - 60_000, true);

    {
        let mut guard = cache.lock().await;
        guard.update_candle(Timeframe::M1, recent_candle);
    }

    let (event_tx, _) = tokio::sync::broadcast::channel(100);

    let result = GapDetector::detect_and_backfill(
        "binance", "BTCUSDT", &cache, &source, &event_tx, MarketType::Spot,
    ).await.unwrap();

    assert_eq!(result, 0);
}

#[tokio::test]
async fn test_gap_backfill() {
    let mock = MockKlineSource::new();
    let cache = Arc::new(Mutex::new(SymbolCache::new()));

    let now_ms = chrono::Utc::now().timestamp_millis();
    let current_1m_open = (now_ms / 60_000) * 60_000;

    let old_candle = make_1m_candle(current_1m_open - 10 * 60_000, true);
    {
        let mut guard = cache.lock().await;
        guard.update_candle(Timeframe::M1, old_candle);
    }

    let gap_candles: Vec<Candle> = (1..10)
        .map(|i| make_1m_candle(current_1m_open - (10 - i) as i64 * 60_000, true))
        .collect();
    mock.add_data("binance", "BTCUSDT", "1m", gap_candles);

    let source = mock.into_source();
    let (event_tx, _) = tokio::sync::broadcast::channel(100);

    let result = GapDetector::detect_and_backfill(
        "binance", "BTCUSDT", &cache, &source, &event_tx, MarketType::Spot,
    ).await.unwrap();

    assert!(result > 0);
}

#[tokio::test]
async fn test_gap_backfill_empty_response() {
    let source = MockKlineSource::new().into_source();
    let cache = Arc::new(Mutex::new(SymbolCache::new()));

    let now_ms = chrono::Utc::now().timestamp_millis();
    let current_1m_open = (now_ms / 60_000) * 60_000;

    let old_candle = make_1m_candle(current_1m_open - 10 * 60_000, true);
    {
        let mut guard = cache.lock().await;
        guard.update_candle(Timeframe::M1, old_candle);
    }

    let (event_tx, _) = tokio::sync::broadcast::channel(100);

    let result = GapDetector::detect_and_backfill(
        "binance", "BTCUSDT", &cache, &source, &event_tx, MarketType::Spot,
    ).await.unwrap();

    assert_eq!(result, 0);
}

#[tokio::test]
async fn test_check_continuity_empty_cache() {
    let cache = Arc::new(Mutex::new(SymbolCache::new()));
    let report = GapDetector::check_continuity("binance", "BTCUSDT", &cache).await;
    assert!(!report.is_continuous);
    assert_eq!(report.missing_minutes, u32::MAX);
}

#[tokio::test]
async fn test_check_continuity_up_to_date() {
    let cache = Arc::new(Mutex::new(SymbolCache::new()));
    let now_ms = chrono::Utc::now().timestamp_millis();
    let current_1m_open = (now_ms / 60_000) * 60_000;
    let recent_candle = make_1m_candle(current_1m_open - 60_000, true);

    {
        let mut guard = cache.lock().await;
        guard.update_candle(Timeframe::M1, recent_candle);
    }

    let report = GapDetector::check_continuity("binance", "BTCUSDT", &cache).await;
    assert!(report.is_continuous);
    assert_eq!(report.missing_minutes, 0);
}

#[tokio::test]
async fn test_check_continuity_gap_detected() {
    let cache = Arc::new(Mutex::new(SymbolCache::new()));
    let now_ms = chrono::Utc::now().timestamp_millis();
    let current_1m_open = (now_ms / 60_000) * 60_000;
    let old_candle = make_1m_candle(current_1m_open - 60 * 60_000, true);

    {
        let mut guard = cache.lock().await;
        guard.update_candle(Timeframe::M1, old_candle);
    }

    let report = GapDetector::check_continuity("binance", "BTCUSDT", &cache).await;
    assert!(!report.is_continuous);
    assert!(report.missing_minutes > 0);
    assert!(report.gap_start.is_some());
    assert!(report.gap_end.is_some());
}

#[tokio::test]
async fn test_initial_load_event_broadcast() {
    let mock = MockKlineSource::new();
    let now_ms = chrono::Utc::now().timestamp_millis();
    let current_1m_open = (now_ms / 60_000) * 60_000;
    let start_1m = current_1m_open - 100 * 60_000;

    let candles_1m: Vec<Candle> = (0..100)
        .map(|i| make_1m_candle(start_1m + i as i64 * 60_000, true))
        .collect();
    mock.add_data("binance", "BTCUSDT", "1m", candles_1m);

    for tf_str in &["5m", "15m", "1h", "4h", "1d"] {
        mock.add_data("binance", "BTCUSDT", tf_str, vec![]);
    }

    let source = mock.into_source();
    let cache = Arc::new(Mutex::new(SymbolCache::new()));
    let (event_tx, mut event_rx) = tokio::sync::broadcast::channel(100);

    let _ = GapDetector::detect_and_backfill(
        "binance", "BTCUSDT", &cache, &source, &event_tx, MarketType::Spot,
    ).await;

    let event = event_rx.try_recv();
    assert!(event.is_ok());
    let event = event.unwrap();
    assert_eq!(event.event_type, KlineEventType::Backfilled);
    assert_eq!(event.exchange, "binance");
    assert_eq!(event.symbol, "BTCUSDT");
}

// ============================================================
// P0-3: 大间隔回填截断测试 (1 test)
// ============================================================

/// P0-3: When gap > 1000 minutes, the limit passed to source.fetch_klines
/// should be truncated to 1000 (gap_minutes.min(1000)).
#[tokio::test]
async fn test_gap_backfill_truncation_large_gap() {
    use std::sync::atomic::AtomicU32;
    use std::sync::Arc as StdArc;

    // Use a tracking mock that records the limit parameter
    let last_limit = StdArc::new(AtomicU32::new(0));
    let last_limit_clone = last_limit.clone();

    struct TrackingMockSourceInner {
        data: StdMutex<HashMap<String, Vec<Candle>>>,
        last_limit: StdArc<AtomicU32>,
    }

    #[async_trait]
    impl KlineSource for TrackingMockSourceInner {
        async fn fetch_klines(
            &self,
            exchange: &str,
            symbol: &str,
            timeframe: &str,
            limit: u32,
            since: Option<i64>,
            _market_type: Option<MarketType>,
        ) -> anyhow::Result<Vec<Candle>> {
            // Record the limit that was actually requested
            self.last_limit.store(limit, std::sync::atomic::Ordering::Relaxed);

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

    let mut mock = TrackingMockSourceInner {
        data: StdMutex::new(HashMap::new()),
        last_limit: last_limit_clone,
    };

    let cache = Arc::new(Mutex::new(SymbolCache::new()));

    let now_ms = chrono::Utc::now().timestamp_millis();
    let current_1m_open = (now_ms / 60_000) * 60_000;

    // Set last closed 1m to 1500 minutes ago — gap is ~1500 minutes
    let old_candle = make_1m_candle(current_1m_open - 1500 * 60_000, true);
    {
        let mut guard = cache.lock().await;
        guard.update_candle(Timeframe::M1, old_candle);
    }

    // Provide gap data (2000 candles covering the gap)
    let gap_candles: Vec<Candle> = (0..2000)
        .map(|i| make_1m_candle(current_1m_open - 1500 * 60_000 + (i as i64 + 1) * 60_000, true))
        .collect();
    {
        let key = format!("{}:{}:{}", "binance", "BTCUSDT", "1m");
        mock.data.lock().unwrap().insert(key, gap_candles);
    }

    let source: Arc<dyn KlineSource> = Arc::new(mock);
    let (event_tx, _) = tokio::sync::broadcast::channel(100);

    let _ = GapDetector::detect_and_backfill(
        "binance", "BTCUSDT", &cache, &source, &event_tx, MarketType::Spot,
    ).await;

    // The limit should be truncated to 1000, not the full gap size (~1500)
    let recorded_limit = last_limit.load(std::sync::atomic::Ordering::Relaxed);
    assert_eq!(recorded_limit, 1000, "limit should be truncated to 1000 for large gaps");
}

// ============================================================
// P0-4: initial_load unclosed 高级周期替换测试 (1 test)
// ============================================================

/// P0-4: When REST returns closed 5m candles but 1m aggregation produces
/// an unclosed 5m candle with a later open_time, the unclosed should replace.
#[tokio::test]
async fn test_initial_load_unclosed_high_tf_replacement() {
    let mock = MockKlineSource::new();
    let now_ms = chrono::Utc::now().timestamp_millis();
    let current_1m_open = (now_ms / 60_000) * 60_000;

    // 1m data: 10 recent candles, last one is unclosed (current minute)
    let start_1m = current_1m_open - 9 * 60_000;
    let mut candles_1m: Vec<Candle> = (0..9)
        .map(|i| make_1m_candle(start_1m + i as i64 * 60_000, true))
        .collect();
    // Add the current (unclosed) 1m candle
    candles_1m.push(make_1m_candle(current_1m_open, false));
    mock.add_data("binance", "BTCUSDT", "1m", candles_1m);

    // 5m data: return a closed 5m candle whose open_time is BEFORE the
    // unclosed 5m aggregated from 1m data.
    // The current 5m group starts at align_open_time(current_1m_open, M5).
    // The previous closed 5m group starts 5 minutes before that.
    let current_5m_open = align_open_time(current_1m_open, Timeframe::M5);
    let prev_5m_open = current_5m_open - 300_000;

    // Provide a 5m candle for the previous group (closed)
    mock.add_data("binance", "BTCUSDT", "5m", vec![
        make_high_tf_candle(prev_5m_open, Timeframe::M5, true),
    ]);

    // Other timeframes return empty
    for tf_str in &["15m", "1h", "4h", "1d"] {
        mock.add_data("binance", "BTCUSDT", tf_str, vec![]);
    }

    let source = mock.into_source();
    let cache = Arc::new(Mutex::new(SymbolCache::new()));
    let (event_tx, _) = tokio::sync::broadcast::channel(100);

    let result = GapDetector::detect_and_backfill(
        "binance", "BTCUSDT", &cache, &source, &event_tx, MarketType::Spot,
    ).await.unwrap();

    assert!(result > 0, "initial_load should return some candles");

    let guard = cache.lock().await;
    let m5_candles = guard.get_klines(Timeframe::M5);

    // Should have at least the previous closed 5m and the unclosed current 5m
    assert!(m5_candles.len() >= 2, "should have at least 2 5m candles (1 closed + 1 unclosed)");

    // The last 5m candle should be unclosed (from 1m aggregation)
    let last_m5 = m5_candles.last().unwrap();
    assert!(!last_m5.closed, "last 5m candle should be unclosed (aggregated from current 1m)");
    assert_eq!(last_m5.open_time, current_5m_open, "last 5m open_time should match current 5m group");
}
