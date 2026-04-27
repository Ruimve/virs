use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Timeframe {
    #[serde(rename = "1m")]
    M1,
    #[serde(rename = "5m")]
    M5,
    #[serde(rename = "15m")]
    M15,
    #[serde(rename = "1h")]
    H1,
    #[serde(rename = "4h")]
    H4,
    #[serde(rename = "1d")]
    D1,
}

impl Timeframe {
    pub fn all() -> &'static [Timeframe] {
        &[Timeframe::M1, Timeframe::M5, Timeframe::M15, Timeframe::H1, Timeframe::H4, Timeframe::D1]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Timeframe::M1 => "1m",
            Timeframe::M5 => "5m",
            Timeframe::M15 => "15m",
            Timeframe::H1 => "1h",
            Timeframe::H4 => "4h",
            Timeframe::D1 => "1d",
        }
    }

    pub fn ms(&self) -> i64 {
        match self {
            Timeframe::M1 => 60_000,
            Timeframe::M5 => 300_000,
            Timeframe::M15 => 900_000,
            Timeframe::H1 => 3_600_000,
            Timeframe::H4 => 14_400_000,
            Timeframe::D1 => 86_400_000,
        }
    }

    pub fn from_str_lossy(s: &str) -> Option<Self> {
        match s {
            "1m" => Some(Timeframe::M1),
            "5m" => Some(Timeframe::M5),
            "15m" => Some(Timeframe::M15),
            "1h" => Some(Timeframe::H1),
            "4h" => Some(Timeframe::H4),
            "1d" | "1D" => Some(Timeframe::D1),
            _ => None,
        }
    }

    pub fn minutes(&self) -> i64 {
        self.ms() / 60_000
    }

    pub fn default_limit(&self) -> usize {
        match self {
            Timeframe::M1 => 2000,
            Timeframe::M5 | Timeframe::M15 => 1000,
            Timeframe::H1 | Timeframe::H4 => 1000,
            Timeframe::D1 => 1000,
        }
    }
}

impl fmt::Display for Timeframe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub open_time: i64,
    pub close_time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub quote_volume: f64,
    pub trades: i64,
    pub closed: bool,
}

impl Candle {
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn merge(&mut self, other: &Candle) {
        if other.high > self.high {
            self.high = other.high;
        }
        if other.low < self.low {
            self.low = other.low;
        }
        self.close = other.close;
        self.volume += other.volume;
        self.quote_volume += other.quote_volume;
        self.trades += other.trades;
        self.close_time = other.close_time;
        self.closed = other.closed;
    }

    pub fn from_1m(base: &Candle, timeframe: Timeframe) -> Self {
        let tf_ms = timeframe.ms();
        let aligned_open_time = (base.open_time / tf_ms) * tf_ms;
        Candle {
            open_time: aligned_open_time,
            close_time: aligned_open_time + tf_ms - 1,
            open: base.open,
            high: base.high,
            low: base.low,
            close: base.close,
            volume: base.volume,
            quote_volume: base.quote_volume,
            trades: base.trades,
            closed: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KlineEvent {
    pub exchange: String,
    pub symbol: String,
    pub timeframe: Timeframe,
    pub candle: Candle,
    pub event_type: KlineEventType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KlineEventType {
    Update,
    Closed,
    Backfilled,
}

#[derive(Debug, Clone)]
pub struct WsCandleUpdate {
    pub symbol: String,
    pub candle: Candle,
}

#[derive(Debug, Clone)]
pub enum WsEvent {
    Candle(WsCandleUpdate),
    Reconnected,
}

#[async_trait]
pub trait KlineWsClient: Send + Sync {
    async fn start(&mut self, update_tx: broadcast::Sender<WsEvent>);
    async fn stop(&mut self);
    async fn subscribe(&self, symbol: &str);
    async fn unsubscribe(&self, symbol: &str);
    fn is_running(&self) -> bool;
}

#[derive(Debug, Clone, Serialize)]
pub struct AllTimeframesData {
    pub m1: Vec<Candle>,
    pub m5: Vec<Candle>,
    pub m15: Vec<Candle>,
    pub h1: Vec<Candle>,
    pub h4: Vec<Candle>,
    pub d1: Vec<Candle>,
}

#[derive(Debug, Clone)]
pub struct KlineEngineConfig {
    pub reconnect_delay_secs: u64,
    pub max_reconnect_delay_secs: u64,
    pub ws_ping_interval_secs: u64,
    pub ws_max_lifetime_secs: u64,
    pub backfill_on_start: bool,
    pub event_channel_capacity: usize,
    pub proxy_url: Option<String>,
}

impl Default for KlineEngineConfig {
    fn default() -> Self {
        Self {
            reconnect_delay_secs: 1,
            max_reconnect_delay_secs: 60,
            ws_ping_interval_secs: 30,
            ws_max_lifetime_secs: 23 * 3600,
            backfill_on_start: true,
            event_channel_capacity: 8192,
            proxy_url: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BacktestRangeLimit {
    pub timeframe: Timeframe,
    pub max_days: u32,
    pub recommended_days: u32,
    pub estimated_candles: u32,
    pub estimated_1m_required: u32,
}

impl BacktestRangeLimit {
    pub fn for_timeframe(tf: Timeframe) -> Self {
        match tf {
            Timeframe::M1 => BacktestRangeLimit {
                timeframe: tf,
                max_days: 7,
                recommended_days: 3,
                estimated_candles: 7 * 24 * 60,
                estimated_1m_required: 7 * 24 * 60,
            },
            Timeframe::M5 => BacktestRangeLimit {
                timeframe: tf,
                max_days: 30,
                recommended_days: 14,
                estimated_candles: 30 * 24 * 12,
                estimated_1m_required: 30 * 24 * 60,
            },
            Timeframe::M15 => BacktestRangeLimit {
                timeframe: tf,
                max_days: 90,
                recommended_days: 30,
                estimated_candles: 90 * 24 * 4,
                estimated_1m_required: 90 * 24 * 60,
            },
            Timeframe::H1 => BacktestRangeLimit {
                timeframe: tf,
                max_days: 365,
                recommended_days: 90,
                estimated_candles: 365 * 24,
                estimated_1m_required: 365 * 24 * 60,
            },
            Timeframe::H4 => BacktestRangeLimit {
                timeframe: tf,
                max_days: 730,
                recommended_days: 180,
                estimated_candles: 730 * 6,
                estimated_1m_required: 730 * 24 * 60,
            },
            Timeframe::D1 => BacktestRangeLimit {
                timeframe: tf,
                max_days: 1825,
                recommended_days: 365,
                estimated_candles: 1825,
                estimated_1m_required: 1825 * 24 * 60,
            },
        }
    }

    pub fn all_limits() -> Vec<BacktestRangeLimit> {
        Timeframe::all().iter().map(|tf| Self::for_timeframe(*tf)).collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestRangeInfo {
    pub timeframe: String,
    pub max_days: u32,
    pub recommended_days: u32,
    pub estimated_candles: u32,
}

impl From<BacktestRangeLimit> for BacktestRangeInfo {
    fn from(limit: BacktestRangeLimit) -> Self {
        BacktestRangeInfo {
            timeframe: limit.timeframe.to_string(),
            max_days: limit.max_days,
            recommended_days: limit.recommended_days,
            estimated_candles: limit.estimated_candles,
        }
    }
}

pub fn subscription_key(exchange: &str, symbol: &str) -> String {
    format!("{}:{}", exchange.to_lowercase(), symbol.to_uppercase())
}

pub fn align_open_time(open_time: i64, timeframe: Timeframe) -> i64 {
    (open_time / timeframe.ms()) * timeframe.ms()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeframe_ms() {
        assert_eq!(Timeframe::M1.ms(), 60_000);
        assert_eq!(Timeframe::M5.ms(), 300_000);
        assert_eq!(Timeframe::M15.ms(), 900_000);
        assert_eq!(Timeframe::H1.ms(), 3_600_000);
        assert_eq!(Timeframe::H4.ms(), 14_400_000);
        assert_eq!(Timeframe::D1.ms(), 86_400_000);
    }

    #[test]
    fn test_timeframe_as_str() {
        assert_eq!(Timeframe::M1.as_str(), "1m");
        assert_eq!(Timeframe::M5.as_str(), "5m");
        assert_eq!(Timeframe::M15.as_str(), "15m");
        assert_eq!(Timeframe::H1.as_str(), "1h");
        assert_eq!(Timeframe::H4.as_str(), "4h");
        assert_eq!(Timeframe::D1.as_str(), "1d");
    }

    #[test]
    fn test_timeframe_from_str_lossy() {
        assert_eq!(Timeframe::from_str_lossy("1m"), Some(Timeframe::M1));
        assert_eq!(Timeframe::from_str_lossy("5m"), Some(Timeframe::M5));
        assert_eq!(Timeframe::from_str_lossy("15m"), Some(Timeframe::M15));
        assert_eq!(Timeframe::from_str_lossy("1h"), Some(Timeframe::H1));
        assert_eq!(Timeframe::from_str_lossy("4h"), Some(Timeframe::H4));
        assert_eq!(Timeframe::from_str_lossy("1d"), Some(Timeframe::D1));
        assert_eq!(Timeframe::from_str_lossy("1D"), Some(Timeframe::D1));
        assert_eq!(Timeframe::from_str_lossy("2h"), None);
        assert_eq!(Timeframe::from_str_lossy(""), None);
    }

    #[test]
    fn test_timeframe_minutes() {
        assert_eq!(Timeframe::M1.minutes(), 1);
        assert_eq!(Timeframe::M5.minutes(), 5);
        assert_eq!(Timeframe::M15.minutes(), 15);
        assert_eq!(Timeframe::H1.minutes(), 60);
        assert_eq!(Timeframe::H4.minutes(), 240);
        assert_eq!(Timeframe::D1.minutes(), 1440);
    }

    #[test]
    fn test_timeframe_default_limit() {
        assert_eq!(Timeframe::M1.default_limit(), 2000);
        assert_eq!(Timeframe::M5.default_limit(), 1000);
        assert_eq!(Timeframe::M15.default_limit(), 1000);
        assert_eq!(Timeframe::H1.default_limit(), 1000);
        assert_eq!(Timeframe::H4.default_limit(), 1000);
        assert_eq!(Timeframe::D1.default_limit(), 1000);
    }

    #[test]
    fn test_timeframe_all() {
        let all = Timeframe::all();
        assert_eq!(all.len(), 6);
        assert!(all.contains(&Timeframe::M1));
        assert!(all.contains(&Timeframe::D1));
    }

    #[test]
    fn test_timeframe_display() {
        assert_eq!(format!("{}", Timeframe::M1), "1m");
        assert_eq!(format!("{}", Timeframe::H1), "1h");
        assert_eq!(format!("{}", Timeframe::D1), "1d");
    }

    #[test]
    fn test_timeframe_serde() {
        let json = serde_json::to_string(&Timeframe::M1).unwrap();
        assert_eq!(json, "\"1m\"");
        let tf: Timeframe = serde_json::from_str("\"5m\"").unwrap();
        assert_eq!(tf, Timeframe::M5);
    }

    #[test]
    fn test_candle_merge() {
        let mut base = Candle {
            open_time: 0, close_time: 59_999,
            open: 100.0, high: 110.0, low: 95.0, close: 105.0,
            volume: 50.0, quote_volume: 5000.0, trades: 100, closed: false,
        };
        let update = Candle {
            open_time: 0, close_time: 59_999,
            open: 100.0, high: 115.0, low: 90.0, close: 108.0,
            volume: 30.0, quote_volume: 3000.0, trades: 50, closed: true,
        };
        base.merge(&update);
        assert_eq!(base.high, 115.0);
        assert_eq!(base.low, 90.0);
        assert_eq!(base.close, 108.0);
        assert!((base.volume - 80.0).abs() < f64::EPSILON);
        assert!((base.quote_volume - 8000.0).abs() < f64::EPSILON);
        assert_eq!(base.trades, 150);
        assert!(base.closed);
    }

    #[test]
    fn test_candle_merge_no_lower_high() {
        let mut base = Candle {
            open_time: 0, close_time: 59_999,
            open: 100.0, high: 120.0, low: 90.0, close: 105.0,
            volume: 50.0, quote_volume: 5000.0, trades: 100, closed: false,
        };
        let update = Candle {
            open_time: 0, close_time: 59_999,
            open: 100.0, high: 110.0, low: 95.0, close: 108.0,
            volume: 30.0, quote_volume: 3000.0, trades: 50, closed: true,
        };
        base.merge(&update);
        assert_eq!(base.high, 120.0);
        assert_eq!(base.low, 90.0);
    }

    #[test]
    fn test_candle_from_1m() {
        let base = Candle {
            open_time: 3_600_000, close_time: 3_659_999,
            open: 100.0, high: 110.0, low: 95.0, close: 105.0,
            volume: 50.0, quote_volume: 5000.0, trades: 100, closed: true,
        };
        let h1 = Candle::from_1m(&base, Timeframe::H1);
        assert_eq!(h1.open_time, 3_600_000);
        assert_eq!(h1.close_time, 3_600_000 + 3_600_000 - 1);
        assert_eq!(h1.open, 100.0);
        assert_eq!(h1.high, 110.0);
        assert_eq!(h1.low, 95.0);
        assert_eq!(h1.close, 105.0);
        assert!(!h1.closed);
    }

    #[test]
    fn test_candle_from_1m_alignment() {
        let base = Candle {
            open_time: 3_630_000, close_time: 3_689_999,
            open: 100.0, high: 110.0, low: 95.0, close: 105.0,
            volume: 50.0, quote_volume: 5000.0, trades: 100, closed: true,
        };
        let h1 = Candle::from_1m(&base, Timeframe::H1);
        assert_eq!(h1.open_time, 3_600_000);
    }

    #[test]
    fn test_candle_is_closed() {
        let c1 = Candle { open_time: 0, close_time: 0, open: 0.0, high: 0.0, low: 0.0, close: 0.0, volume: 0.0, quote_volume: 0.0, trades: 0, closed: true };
        let c2 = Candle { open_time: 0, close_time: 0, open: 0.0, high: 0.0, low: 0.0, close: 0.0, volume: 0.0, quote_volume: 0.0, trades: 0, closed: false };
        assert!(c1.is_closed());
        assert!(!c2.is_closed());
    }

    #[test]
    fn test_align_open_time() {
        assert_eq!(align_open_time(0, Timeframe::M5), 0);
        assert_eq!(align_open_time(60_000, Timeframe::M5), 0);
        assert_eq!(align_open_time(300_000, Timeframe::M5), 300_000);
        assert_eq!(align_open_time(3_600_000, Timeframe::H1), 3_600_000);
        assert_eq!(align_open_time(3_630_000, Timeframe::H1), 3_600_000);
        assert_eq!(align_open_time(86_400_000, Timeframe::D1), 86_400_000);
        assert_eq!(align_open_time(90_000_000, Timeframe::D1), 86_400_000);
    }

    #[test]
    fn test_subscription_key() {
        assert_eq!(subscription_key("Binance", "btcusdt"), "binance:BTCUSDT");
        assert_eq!(subscription_key("OKX", "BTC/USDT"), "okx:BTC/USDT");
    }

    #[test]
    fn test_kline_event_type_serde() {
        assert_eq!(serde_json::to_string(&KlineEventType::Update).unwrap(), "\"Update\"");
        assert_eq!(serde_json::to_string(&KlineEventType::Closed).unwrap(), "\"Closed\"");
        assert_eq!(serde_json::to_string(&KlineEventType::Backfilled).unwrap(), "\"Backfilled\"");
    }

    #[test]
    fn test_backtest_range_limit() {
        let m1 = BacktestRangeLimit::for_timeframe(Timeframe::M1);
        assert_eq!(m1.max_days, 7);
        assert_eq!(m1.recommended_days, 3);
        assert!(m1.estimated_candles > 0);

        let d1 = BacktestRangeLimit::for_timeframe(Timeframe::D1);
        assert_eq!(d1.max_days, 1825);
        assert!(d1.estimated_1m_required > d1.estimated_candles);
    }

    #[test]
    fn test_kline_engine_config_default() {
        let config = KlineEngineConfig::default();
        assert!(config.backfill_on_start);
        assert_eq!(config.event_channel_capacity, 8192);
        assert_eq!(config.reconnect_delay_secs, 1);
    }
}
