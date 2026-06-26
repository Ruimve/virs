//! Market data types for virs-market service.
//!
//! Re-exports core types from virs-ccxt and defines market-service-specific types.

use serde::{Deserialize, Serialize};
use std::fmt;

// Re-export kline types from ccxt
pub use virs_ccxt::ws_types::{Candle, KlineWsClient, WsCandleUpdate, WsEvent};

// Re-export order book types from ccxt
pub use virs_ccxt::ws_types::{
    OrderBookLevel, OrderBookWsClient, WsOrderBookEvent, WsOrderBookUpdate,
};

// Re-export MarketType from virs-types
pub use virs_types::enums::MarketType;

/// Timeframe enum — defines supported candle intervals.
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
        &[
            Timeframe::M1,
            Timeframe::M5,
            Timeframe::M15,
            Timeframe::H1,
            Timeframe::H4,
            Timeframe::D1,
        ]
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
        1000
    }
}

impl fmt::Display for Timeframe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Kline event emitted by KlineEngine.
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

/// All timeframes data for a symbol.
#[derive(Debug, Clone, Serialize)]
pub struct AllTimeframesData {
    pub m1: Vec<Candle>,
    pub m5: Vec<Candle>,
    pub m15: Vec<Candle>,
    pub h1: Vec<Candle>,
    pub h4: Vec<Candle>,
    pub d1: Vec<Candle>,
}

/// KlineEngine configuration.
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

/// Backtest range limit per timeframe.
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
        Timeframe::all()
            .iter()
            .map(|tf| Self::for_timeframe(*tf))
            .collect()
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

/// Kline data source — fetches klines from exchange REST API.
#[async_trait::async_trait]
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

/// Kline persistence — saves/loads candles to/from storage.
#[async_trait::async_trait]
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

pub fn subscription_key(exchange: &str, symbol: &str) -> String {
    format!("{}:{}", exchange.to_lowercase(), symbol.to_uppercase())
}

pub fn align_open_time(open_time: i64, timeframe: Timeframe) -> i64 {
    (open_time / timeframe.ms()) * timeframe.ms()
}

// ============================================================
// OrderBook engine types
// ============================================================

/// OrderBook event emitted by OrderBookEngine.
/// Mirrors KlineEvent structure but for order book snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookEvent {
    pub exchange: String,
    pub symbol: String,
    pub bids: Vec<OrderBookLevel>,
    pub asks: Vec<OrderBookLevel>,
    pub timestamp: i64,
}

/// OrderBookEngine configuration.
#[derive(Debug, Clone)]
pub struct OrderBookEngineConfig {
    pub event_channel_capacity: usize,
}

impl Default for OrderBookEngineConfig {
    fn default() -> Self {
        Self {
            event_channel_capacity: 1024,
        }
    }
}
