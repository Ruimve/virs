use std::sync::Arc;

use tokio::sync::Mutex;
use tracing;

use super::cache::SymbolCache;
use super::types::{Candle, Timeframe, KlineEvent, KlineEventType, MarketType, KlineSource, align_open_time};
use super::aggregator::Aggregator;

pub struct GapDetector;

const INITIAL_1M_LIMIT: u32 = 2000;
const INITIAL_HIGH_TF_LIMIT: u32 = 1000;

impl GapDetector {
    pub async fn detect_and_backfill(
        exchange: &str,
        symbol: &str,
        cache: &Arc<Mutex<SymbolCache>>,
        source: &Arc<dyn KlineSource>,
        event_tx: &tokio::sync::broadcast::Sender<KlineEvent>,
        market_type: MarketType,
    ) -> anyhow::Result<usize> {
        let last_closed_1m = {
            let guard = cache.lock().await;
            guard.last_closed_1m()
        };

        let now_ms = chrono::Utc::now().timestamp_millis();
        let expected_next = match last_closed_1m {
            Some(c) => c.open_time + 60_000,
            None => {
                tracing::info!("[GapDetector] No 1m candles in cache for {}/{}, loading initial data", exchange, symbol);
                return Self::initial_load(exchange, symbol, cache, source, event_tx, market_type).await;
            }
        };

        let current_1m_open = (now_ms / 60_000) * 60_000;
        if expected_next >= current_1m_open {
            tracing::debug!("[GapDetector] No gap for {}/{}", exchange, symbol);
            return Ok(0);
        }

        let gap_start = expected_next;
        let gap_end = current_1m_open;
        let gap_minutes = ((gap_end - gap_start) / 60_000) as u32;

        tracing::info!(
            "[GapDetector] Gap detected for {}/{}: {} minutes ({} to {})",
            exchange, symbol, gap_minutes, gap_start, gap_end
        );

        let limit = gap_minutes.min(1000);
        let fetched = source.fetch_klines(exchange, symbol, "1m", limit, Some(gap_start), Some(market_type)).await?;

        if fetched.is_empty() {
            tracing::warn!("[GapDetector] No data returned for gap backfill: {}/{}", exchange, symbol);
            return Ok(0);
        }

        let mut backfilled_count = 0;
        let aggregated_data: Vec<(Timeframe, Vec<Candle>)> = {
            let mut guard = cache.lock().await;

            for candle in &fetched {
                if candle.open_time >= gap_start && candle.open_time < gap_end && candle.closed {
                    guard.update_candle(Timeframe::M1, candle.clone());
                    backfilled_count += 1;
                }
            }

            if backfilled_count > 0 {
                let all_1m = guard.get_klines(Timeframe::M1);
                [Timeframe::M5, Timeframe::M15, Timeframe::H1, Timeframe::H4, Timeframe::D1]
                    .iter()
                    .filter_map(|&tf| {
                        let aggregated = Aggregator::aggregate_1m_to_timeframe(&all_1m, tf);
                        if aggregated.is_empty() { None } else { Some((tf, aggregated)) }
                    })
                    .collect()
            } else {
                Vec::new()
            }
        };

        if !aggregated_data.is_empty() {
            let mut guard = cache.lock().await;
            for (tf, candles) in &aggregated_data {
                guard.replace_timeframe(*tf, candles.clone());
            }
        }

        if backfilled_count > 0 {
            let _ = event_tx.send(KlineEvent {
                exchange: exchange.to_string(),
                symbol: symbol.to_string(),
                timeframe: Timeframe::M1,
                candle: fetched.last().cloned().unwrap_or_else(|| Candle {
                    open_time: 0, close_time: 0, open: 0.0, high: 0.0, low: 0.0,
                    close: 0.0, volume: 0.0, quote_volume: 0.0, trades: 0, closed: false,
                }),
                event_type: KlineEventType::Backfilled,
            });
        }

        tracing::info!("[GapDetector] Backfilled {} candles for {}/{}", backfilled_count, exchange, symbol);
        Ok(backfilled_count)
    }

    async fn initial_load(
        exchange: &str,
        symbol: &str,
        cache: &Arc<Mutex<SymbolCache>>,
        source: &Arc<dyn KlineSource>,
        event_tx: &tokio::sync::broadcast::Sender<KlineEvent>,
        market_type: MarketType,
    ) -> anyhow::Result<usize> {
        tracing::info!(
            "[GapDetector] Initial load for {}/{}: 1m={} + high_tf={}",
            exchange, symbol, INITIAL_1M_LIMIT, INITIAL_HIGH_TF_LIMIT
        );

        let (result_1m, results_high): (anyhow::Result<Vec<Candle>>, Vec<(Timeframe, anyhow::Result<Vec<Candle>>)>) = {
            let fetch_1m = source.fetch_klines(exchange, symbol, "1m", INITIAL_1M_LIMIT, None, Some(market_type.clone()));
            let fetch_m5 = source.fetch_klines(exchange, symbol, "5m", INITIAL_HIGH_TF_LIMIT, None, Some(market_type.clone()));
            let fetch_m15 = source.fetch_klines(exchange, symbol, "15m", INITIAL_HIGH_TF_LIMIT, None, Some(market_type.clone()));
            let fetch_h1 = source.fetch_klines(exchange, symbol, "1h", INITIAL_HIGH_TF_LIMIT, None, Some(market_type.clone()));
            let fetch_h4 = source.fetch_klines(exchange, symbol, "4h", INITIAL_HIGH_TF_LIMIT, None, Some(market_type.clone()));
            let fetch_d1 = source.fetch_klines(exchange, symbol, "1d", INITIAL_HIGH_TF_LIMIT, None, Some(market_type.clone()));

            let (r_1m, r_m5, r_m15, r_h1, r_h4, r_d1) = tokio::join!(fetch_1m, fetch_m5, fetch_m15, fetch_h1, fetch_h4, fetch_d1);

            let high = vec![
                (Timeframe::M5, r_m5),
                (Timeframe::M15, r_m15),
                (Timeframe::H1, r_h1),
                (Timeframe::H4, r_h4),
                (Timeframe::D1, r_d1),
            ];

            (r_1m, high)
        };

        let candles_1m = match result_1m {
            Ok(c) if !c.is_empty() => c,
            Ok(_) => {
                tracing::warn!("[GapDetector] No 1m candles for {}/{}", exchange, symbol);
                return Ok(0);
            }
            Err(e) => {
                tracing::error!("[GapDetector] Failed to load 1m candles for {}/{}: {}", exchange, symbol, e);
                return Err(e);
            }
        };

        tracing::info!(
            "[GapDetector] Loaded {} 1m candles for {}/{}",
            candles_1m.len(), exchange, symbol
        );

        let now_ms = chrono::Utc::now().timestamp_millis();
        let current_1m_open = (now_ms / 60_000) * 60_000;

        let unclosed_high: Vec<(Timeframe, Candle)> = [Timeframe::M5, Timeframe::M15, Timeframe::H1, Timeframe::H4, Timeframe::D1]
            .iter()
            .filter_map(|&tf| {
                let _tf_ms = tf.ms();
                let current_tf_open = align_open_time(current_1m_open, tf);
                let relevant: Vec<Candle> = candles_1m.iter()
                    .filter(|c| c.open_time >= current_tf_open)
                    .cloned()
                    .collect();
                if relevant.is_empty() {
                    return None;
                }
                let mut agg = Aggregator::aggregate_1m_to_timeframe(&relevant, tf);
                if let Some(last) = agg.last_mut() {
                    last.closed = false;
                }
                agg.last().cloned().map(|c| (tf, c))
            })
            .collect();

        let mut total = 0;
        {
            let mut guard = cache.lock().await;

            guard.replace_timeframe(Timeframe::M1, candles_1m.clone());
            total += guard.get_klines(Timeframe::M1).len();

            for (tf, result) in &results_high {
                match result {
                    Ok(candles) if !candles.is_empty() => {
                        let mut final_candles = candles.clone();

                        if let Some(pos) = final_candles.iter().rposition(|c| !c.closed) {
                            final_candles.truncate(pos);
                        }

                        if let Some(unclosed) = unclosed_high.iter().find(|(t, _)| *t == *tf) {
                            if let Some(last_rest) = final_candles.last() {
                                if last_rest.open_time < unclosed.1.open_time {
                                    final_candles.push(unclosed.1.clone());
                                } else if last_rest.open_time == unclosed.1.open_time {
                                    let len = final_candles.len();
                                    final_candles[len - 1] = unclosed.1.clone();
                                }
                            } else {
                                final_candles.push(unclosed.1.clone());
                            }
                        }

                        tracing::info!(
                            "[GapDetector] Loaded {} {} candles ({} from REST + unclosed) for {}/{}",
                            final_candles.len(), tf.as_str(), candles.len(), exchange, symbol
                        );
                        guard.replace_timeframe(*tf, final_candles);
                        total += guard.get_klines(*tf).len();
                    }
                    Ok(_) => {
                        tracing::warn!("[GapDetector] No {} candles for {}/{}", tf.as_str(), exchange, symbol);
                        if let Some(unclosed) = unclosed_high.iter().find(|(t, _)| *t == *tf) {
                            guard.replace_timeframe(*tf, vec![unclosed.1.clone()]);
                            total += 1;
                        }
                    }
                    Err(e) => {
                        tracing::error!("[GapDetector] Failed to load {} candles for {}/{}: {}", tf.as_str(), exchange, symbol, e);
                        if let Some(unclosed) = unclosed_high.iter().find(|(t, _)| *t == *tf) {
                            guard.replace_timeframe(*tf, vec![unclosed.1.clone()]);
                            total += 1;
                        }
                    }
                }
            }
        }

        if total > 0 {
            let _ = event_tx.send(KlineEvent {
                exchange: exchange.to_string(),
                symbol: symbol.to_string(),
                timeframe: Timeframe::M1,
                candle: candles_1m.last().cloned().unwrap_or_else(|| Candle {
                    open_time: 0, close_time: 0, open: 0.0, high: 0.0, low: 0.0,
                    close: 0.0, volume: 0.0, quote_volume: 0.0, trades: 0, closed: false,
                }),
                event_type: KlineEventType::Backfilled,
            });
        }

        Ok(total)
    }

    pub async fn check_continuity(
        _exchange: &str,
        _symbol: &str,
        cache: &Arc<Mutex<SymbolCache>>,
    ) -> ContinuityReport {
        let guard = cache.lock().await;
        let last_closed = guard.last_closed_1m();

        let now_ms = chrono::Utc::now().timestamp_millis();
        let current_1m_open = (now_ms / 60_000) * 60_000;

        match last_closed {
            None => ContinuityReport {
                is_continuous: false,
                gap_start: None,
                gap_end: Some(current_1m_open),
                missing_minutes: u32::MAX,
            },
            Some(c) => {
                let expected_next = c.open_time + 60_000;
                if expected_next >= current_1m_open {
                    ContinuityReport {
                        is_continuous: true,
                        gap_start: None,
                        gap_end: None,
                        missing_minutes: 0,
                    }
                } else {
                    ContinuityReport {
                        is_continuous: false,
                        gap_start: Some(expected_next),
                        gap_end: Some(current_1m_open),
                        missing_minutes: ((current_1m_open - expected_next) / 60_000) as u32,
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct ContinuityReport {
    pub is_continuous: bool,
    #[allow(dead_code)]
    pub gap_start: Option<i64>,
    #[allow(dead_code)]
    pub gap_end: Option<i64>,
    pub missing_minutes: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex as StdMutex;
    use async_trait::async_trait;
    use super::super::KlineSource;

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
}
