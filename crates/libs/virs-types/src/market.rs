use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::enums::{MarginMode, PositionSide};


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ticker {
    pub symbol: String,
    pub exchange: String,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub last: f64,
    pub high_24h: f64,
    pub low_24h: f64,
    pub volume_24h: f64,
    pub price_change_24h: f64,
    pub price_change_pct_24h: f64,
    pub timestamp: DateTime<Utc>,
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Kline {
    pub open_time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub close_time: i64,
    pub quote_volume: f64,
    pub trades: i64,
    pub symbol: String,
    pub exchange: String,
    pub interval: String,
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderBook {
    pub symbol: String,
    pub bids: Vec<(f64, f64)>,
    pub asks: Vec<(f64, f64)>,
    pub timestamp: DateTime<Utc>,
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Balance {
    pub asset: String,
    pub free: f64,
    pub used: f64,
    pub total: f64,
}

impl Balance {

    pub fn compute_total(&self) -> f64 {
        self.free + self.used
    }
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExchangePosition {
    pub symbol: String,
    pub side: PositionSide,
    pub quantity: f64,
    pub entry_price: f64,
    pub margin_mode: MarginMode,
    pub info: serde_json::Value,
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FundingRate {
    pub symbol: String,
    pub rate: f64,
    pub next_funding_time: Option<DateTime<Utc>>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiRestrictions {
    pub ip_restrict: bool,
    pub ip_whitelist: Vec<String>,
    pub ip_not_restricted: bool,
    pub create_sub_account: bool,
    pub read_info: bool,
    pub enable_withdrawals: bool,
    pub enable_internal_transfer: bool,
    pub enable_futures: bool,
    pub enable_vanilla_options: bool,
    pub enable_portfolio_margin_trading: bool,
    pub enable_fix_api_trade: bool,
    pub enable_fix_api_read: bool,
    pub info: serde_json::Value,
}
