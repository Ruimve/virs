use crate::engine::kline::*;
use std::collections::HashMap;
use std::sync::Mutex as StdMutex;
use async_trait::async_trait;

// === Mock KlineSource (superset with errors support) ===

pub(super) struct MockKlineSource {
    data: StdMutex<HashMap<String, Vec<Candle>>>,
    errors: StdMutex<HashMap<String, String>>,
}

impl MockKlineSource {
    pub(super) fn new() -> Self {
        Self {
            data: StdMutex::new(HashMap::new()),
            errors: StdMutex::new(HashMap::new()),
        }
    }

    pub(super) fn add_data(&self, exchange: &str, symbol: &str, timeframe: &str, candles: Vec<Candle>) {
        let key = format!("{}:{}:{}", exchange, symbol, timeframe);
        self.data.lock().unwrap().insert(key, candles);
    }

    pub(super) fn set_error(&self, exchange: &str, symbol: &str, timeframe: &str, error: &str) {
        let key = format!("{}:{}:{}", exchange, symbol, timeframe);
        self.errors.lock().unwrap().insert(key, error.to_string());
    }

    pub(super) fn into_source(self) -> Arc<dyn KlineSource> {
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

// === Helper functions ===

pub(super) fn make_1m_candle(open_time: i64, closed: bool) -> Candle {
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

pub(super) fn make_1m_sequence(start_time: i64, count: usize, closed: bool) -> Vec<Candle> {
    (0..count)
        .map(|i| make_1m_candle(start_time + i as i64 * 60_000, closed))
        .collect()
}

pub(super) fn make_high_tf_candle(open_time: i64, tf: Timeframe, closed: bool) -> Candle {
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
