use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::exchange::MarginMode;
use crate::position::PositionSide;
use super::enums::{KlineEventType, Timeframe};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketInfo {
    pub id: String,
    pub symbol: String,
    pub base: String,
    pub quote: String,
    pub active: bool,
    pub market_type: crate::exchange::MarketType,
    pub min_amount: Option<f64>,
    pub max_amount: Option<f64>,
    pub min_price: Option<f64>,
    pub max_price: Option<f64>,
    pub min_cost: Option<f64>,
    pub price_precision: Option<u32>,
    pub amount_precision: Option<u32>,
    pub info: serde_json::Value,
}


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


/*
 * 订单簿结构体：包含买卖盘快照和 last_update_id。
 * last_update_id 用于增量更新同步校验，确保本地订单簿与交易所一致。
 */
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderBook {
    pub symbol: String,
    pub bids: Vec<(f64, f64)>,
    pub asks: Vec<(f64, f64)>,
    pub timestamp: DateTime<Utc>,

    /* 最后更新 ID：用于订单簿增量同步校验 */
    pub last_update_id: Option<u64>,
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Balance {
    pub asset: String,
    pub free: f64,
    pub used: f64,
    pub total: f64,
}

impl Balance {

    /* 计算总余额：可用余额 + 已用余额 */
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


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KlineEvent {
    pub exchange: String,
    pub symbol: String,
    pub timeframe: Timeframe,
    pub candle: Candle,
    pub event_type: KlineEventType,
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FundingRate {
    pub symbol: String,
    pub rate: f64,
    pub next_funding_time: Option<DateTime<Utc>>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiRestrictions {
    pub ip_restrict: Option<bool>,
    pub ip_whitelist: Vec<String>,
    pub ip_not_restricted: Option<bool>,
    pub create_sub_account: Option<bool>,
    pub read_info: Option<bool>,
    pub enable_withdrawals: Option<bool>,
    pub enable_internal_transfer: Option<bool>,
    pub enable_futures: Option<bool>,
    pub enable_vanilla_options: Option<bool>,
    pub enable_portfolio_margin_trading: Option<bool>,
    pub enable_fix_api_trade: Option<bool>,
    pub enable_fix_api_read: Option<bool>,
    pub info: serde_json::Value,
}
