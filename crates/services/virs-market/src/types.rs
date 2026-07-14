use serde::{Deserialize, Serialize};
use std::fmt;
use virs_error::VirsResult;


pub use virs_ccxt::ws_types::{Candle, KlineWsClient, WsCandleUpdate, WsEvent};


pub use virs_ccxt::ws_types::{
    OrderBookLevel, OrderBookWsClient, WsOrderBookEvent, WsOrderBookUpdate,
};


pub use virs_types::enums::MarketType;


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

    pub fn default_limit(&self) -> usize {
        1000
    }
}

impl fmt::Display for Timeframe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
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
            event_channel_capacity: 512,
            proxy_url: None,
        }
    }
}


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
    ) -> VirsResult<Vec<Candle>>;
}


#[async_trait::async_trait]
pub trait KlinePersistence: Send + Sync {
    async fn save_candles(
        &self,
        exchange: &str,
        symbol: &str,
        timeframe: &str,
        candles: &[Candle],
    ) -> VirsResult<()>;
}

pub fn subscription_key(exchange: &str, symbol: &str) -> String {
    format!("{}:{}", exchange.to_lowercase(), symbol.to_uppercase())
}

pub fn align_open_time(open_time: i64, timeframe: Timeframe) -> i64 {
    (open_time / timeframe.ms()) * timeframe.ms()
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookEvent {
    pub exchange: String,
    pub symbol: String,
    pub bids: Vec<OrderBookLevel>,
    pub asks: Vec<OrderBookLevel>,
    pub timestamp: i64,
}


#[derive(Debug, Clone)]
pub struct OrderBookEngineConfig {
    pub event_channel_capacity: usize,
}

impl Default for OrderBookEngineConfig {
    fn default() -> Self {
        Self {
            event_channel_capacity: 512,
        }
    }
}
