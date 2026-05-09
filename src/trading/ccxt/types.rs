//! CCXT-style unified market data types.
//!
//! These types are the internal representation used by the ccxt module.
//! They are designed to be exchange-agnostic and map to/from each exchange's
//! native format. The public API layer (models/) re-exports its own types
//! that may differ slightly.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Unified market type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum MarketType {
    Spot,
    Perpetual,
}

/// Unified order side.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Buy,
    Sell,
}

/// Unified order type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum OrderType {
    Market,
    Limit,
    StopMarket,
    StopLimit,
}

/// Unified order status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum OrderStatus {
    Open,
    PartiallyFilled,
    Filled,
    Canceled,
    Expired,
    Failed,
    Rejected,
}

/// Unified ticker data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticker {
    pub symbol: String,
    pub exchange: String,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub last: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub volume: Option<f64>,
    pub quote_volume: Option<f64>,
    pub open: Option<f64>,
    pub close: Option<f64>,
    pub previous_close: Option<f64>,
    pub price_change: Option<f64>,
    pub price_change_pct: Option<f64>,
    pub timestamp: Option<DateTime<Utc>>,
    pub info: serde_json::Value,
}

/// Unified kline/candlestick data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Kline {
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub quote_volume: Option<f64>,
    pub trades: Option<i64>,
}

/// Unified order book.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBook {
    pub symbol: String,
    pub bids: Vec<(f64, f64)>, // (price, amount)
    pub asks: Vec<(f64, f64)>,
    pub timestamp: Option<DateTime<Utc>>,
    pub nonce: Option<u64>,
}

/// Unified balance entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    pub asset: String,
    pub free: f64,
    pub used: f64,
    pub total: f64,
}

/// Unified order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub client_order_id: Option<String>,
    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    pub price: Option<f64>,
    pub amount: f64,
    pub cost: Option<f64>,
    pub filled: f64,
    pub remaining: f64,
    pub status: OrderStatus,
    pub fee: Option<OrderFee>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub info: serde_json::Value,
}

/// Order fee information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderFee {
    pub cost: f64,
    pub currency: String,
    pub rate: Option<f64>,
}

/// Market info / trading rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketInfo {
    pub id: String,           // exchange-native symbol, e.g. "BTCUSDT"
    pub symbol: String,       // unified symbol, e.g. "BTC/USDT"
    pub base: String,         // base asset, e.g. "BTC"
    pub quote: String,        // quote asset, e.g. "USDT"
    pub active: bool,
    pub market_type: MarketType,
    pub min_amount: Option<f64>,
    pub max_amount: Option<f64>,
    pub min_price: Option<f64>,
    pub max_price: Option<f64>,
    pub min_cost: Option<f64>,
    pub price_precision: Option<u32>,
    pub amount_precision: Option<u32>,
    pub info: serde_json::Value,
}

/// Parameters for placing an order.
#[derive(Debug, Clone)]
pub struct PlaceOrderParams {
    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    pub amount: f64,
    pub price: Option<f64>,
    pub market_type: MarketType,
    pub client_order_id: Option<String>,
    pub stop_price: Option<f64>,
    pub time_in_force: Option<TimeInForce>,
    pub reduce_only: Option<bool>,
    /// Leverage multiplier for perpetual orders (e.g., 5 for 5x).
    pub leverage: Option<u32>,
    /// Margin mode for perpetual orders.
    pub margin_mode: Option<MarginMode>,
    /// Position side for hedge mode (LONG/SHORT). None for one-way mode.
    pub position_side: Option<PositionSide>,
}

/// Margin mode for perpetual contracts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum MarginMode {
    Cross,
    Isolated,
}

/// Position side for hedge mode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "UPPERCASE")]
pub enum PositionSide {
    Long,
    Short,
}

/// Funding rate info for perpetual contracts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingRate {
    pub symbol: String,
    pub rate: f64,
    pub next_funding_time: Option<DateTime<Utc>>,
    pub info: serde_json::Value,
}

/// Historical funding rate entry for backtesting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingHistoryEntry {
    pub funding_time: i64,  // Unix timestamp in milliseconds
    pub rate: f64,          // Funding rate (e.g. 0.0001 = 0.01%)
}

/// Position info from exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub side: PositionSide,
    pub size: f64,
    pub entry_price: f64,
    pub leverage: u32,
    pub unrealized_pnl: f64,
    pub margin_mode: MarginMode,
    pub liquidation_price: Option<f64>,
    pub info: serde_json::Value,
}

/// Time in force for orders.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TimeInForce {
    Gtc, // Good Till Cancel
    Ioc, // Immediate Or Cancel
    Fok, // Fill Or Kill
    Poc, // Post Only
}

/// Exchange rate limit info.
#[derive(Debug, Clone)]
pub struct RateLimit {
    pub max_requests_per_second: f64,
    pub max_requests_per_minute: Option<f32>,
}

/// Exchange capabilities / features.
#[derive(Debug, Clone)]
pub struct ExchangeCapabilities {
    pub has: ExchangeFeatures,
    pub rate_limit: RateLimit,
    pub timeframes: Vec<(String, String)>, // (unified, native)
}

/// Supported features flags.
#[derive(Debug, Clone, Default)]
pub struct ExchangeFeatures {
    pub spot: bool,
    pub futures: bool,
    pub perpetual: bool,
    pub fetch_ticker: bool,
    pub fetch_tickers: bool,
    pub fetch_order_book: bool,
    pub fetch_ohlcv: bool,
    pub fetch_balance: bool,
    pub create_order: bool,
    pub cancel_order: bool,
    pub fetch_order: bool,
    pub fetch_open_orders: bool,
    pub fetch_markets: bool,
}
