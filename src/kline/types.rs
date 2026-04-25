use serde::{Deserialize, Serialize};
use std::fmt;

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
            Timeframe::M1 | Timeframe::M5 | Timeframe::M15 | Timeframe::H1 | Timeframe::H4 => 500,
            Timeframe::D1 => 365,
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

impl From<crate::models::Kline> for Candle {
    fn from(k: crate::models::Kline) -> Self {
        Candle {
            open_time: k.open_time,
            close_time: k.close_time,
            open: k.open,
            high: k.high,
            low: k.low,
            close: k.close,
            volume: k.volume,
            quote_volume: k.quote_volume,
            trades: k.trades,
            closed: true,
        }
    }
}

impl From<Candle> for crate::models::Kline {
    fn from(c: Candle) -> Self {
        crate::models::Kline {
            open_time: c.open_time,
            open: c.open,
            high: c.high,
            low: c.low,
            close: c.close,
            volume: c.volume,
            close_time: c.close_time,
            quote_volume: c.quote_volume,
            trades: c.trades,
            symbol: String::new(),
            exchange: String::new(),
            interval: String::new(),
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
    pub ws_base_url_spot: String,
    pub ws_base_url_perpetual: String,
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
            ws_base_url_spot: "wss://stream.binance.com/ws".to_string(),
            ws_base_url_perpetual: "wss://fstream.binance.com/ws".to_string(),
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

pub fn binance_ws_symbol(symbol: &str) -> String {
    symbol.replace('/', "").to_lowercase()
}

pub fn align_open_time(open_time: i64, timeframe: Timeframe) -> i64 {
    (open_time / timeframe.ms()) * timeframe.ms()
}
