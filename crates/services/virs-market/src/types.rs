use serde::{Deserialize, Serialize};
use virs_error::VirsResult;

pub(crate) use virs_type::ws_types::{KlineWsClient, WsEvent};

use virs_type::{Candle, Timeframe};

pub(crate) use virs_type::ws_types::{
    OrderBookLevel, OrderBookWsClient, WsOrderBookEvent,
};

pub(crate) use virs_type::MarketType;

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
