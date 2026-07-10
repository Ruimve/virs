//! CCXT-style unified market data types.
//!
//! Core types (Side, MarketType, OrderType, OrderStatus, Ticker, Kline, etc.)
//! are imported from virs-types to avoid duplication.
//! Only exchange-protocol-specific types remain here.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use virs_error::ExchangeError;

// Re-export all shared types from virs-types
pub use virs_types::enums::{MarketType, OrderStatus, OrderType, PositionMode, PositionSide, Side};
pub use virs_types::market::{
    Balance, ExchangePosition, FundingHistoryEntry, FundingRate, Kline, OrderBook, Ticker,
};

// ---- CCXT-specific types (not shared with application layer) ----

/// Market info / trading rules (exchange-specific, not in virs-types).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketInfo {
    pub id: String,     // exchange-native symbol, e.g. "BTCUSDT"
    pub symbol: String, // unified symbol, e.g. "BTC/USDT"
    pub base: String,   // base asset, e.g. "BTC"
    pub quote: String,  // quote asset, e.g. "USDT"
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

/// Parameters for placing an order (exchange-protocol-specific).
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
    pub leverage: Option<u32>,
    pub margin_mode: Option<MarginMode>,
    pub position_side: Option<PositionSide>,
}

/// Margin mode for perpetual contracts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum MarginMode {
    Cross,
    Isolated,
}

/// Order fee information (exchange-protocol-specific).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderFee {
    pub cost: f64,
    pub currency: String,
    pub rate: Option<f64>,
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

/// Position info from exchange (ccxt-internal, with extra `info` and `margin_mode`).
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

// ---- CCXT-internal order representation ----
// The ccxt layer needs its own Order type because exchange responses
// include `info: serde_json::Value` (raw exchange data) and `fee: Option<OrderFee>`,
// which differ from the application-level Order in virs-models.

/// CCXT-internal order (with raw exchange data).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CcxtOrder {
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
    pub status: CcxtOrderStatus,
    pub fee: Option<OrderFee>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub info: serde_json::Value,
}

/// CCXT-internal order status (includes exchange-specific variants).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum CcxtOrderStatus {
    Open,
    PartiallyFilled,
    Filled,
    Canceled,
    Expired,
    Failed,
    Rejected,
}

impl From<CcxtOrderStatus> for OrderStatus {
    fn from(s: CcxtOrderStatus) -> Self {
        match s {
            CcxtOrderStatus::Open => OrderStatus::Open,
            CcxtOrderStatus::PartiallyFilled => OrderStatus::PartiallyFilled,
            CcxtOrderStatus::Filled => OrderStatus::Filled,
            CcxtOrderStatus::Canceled => OrderStatus::Canceled,
            CcxtOrderStatus::Expired => OrderStatus::Canceled,
            CcxtOrderStatus::Failed => OrderStatus::Failed,
            CcxtOrderStatus::Rejected => OrderStatus::Failed,
        }
    }
}

/// CCXT-internal ticker (fields are Option because exchanges may omit them).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CcxtTicker {
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

impl TryFrom<CcxtTicker> for Ticker {
    type Error = ExchangeError;

    fn try_from(t: CcxtTicker) -> Result<Self, Self::Error> {
        let symbol = t.symbol.clone();

        // bid/ask are optional — not all exchanges return them (e.g. Binance
        // USD-M Futures ticker omits bidPrice/askPrice). No error, just None.
        let bid = t.bid;
        let ask = t.ask;

        // last is mandatory — used for trading decisions and risk checks.
        let last = t.last.ok_or_else(|| {
            tracing::error!(symbol = %symbol, "Ticker last missing");
            ExchangeError::no_data(format!("Ticker last missing for {}", symbol))
        })?;
        let high_24h = t.high.ok_or_else(|| {
            tracing::warn!(symbol = %symbol, "Ticker high_24h missing");
            ExchangeError::no_data(format!("Ticker high_24h missing for {}", symbol))
        })?;
        let low_24h = t.low.ok_or_else(|| {
            tracing::warn!(symbol = %symbol, "Ticker low_24h missing");
            ExchangeError::no_data(format!("Ticker low_24h missing for {}", symbol))
        })?;
        let volume_24h = t.volume.ok_or_else(|| {
            tracing::warn!(symbol = %symbol, "Ticker volume_24h missing");
            ExchangeError::no_data(format!("Ticker volume_24h missing for {}", symbol))
        })?;
        let price_change_24h = t.price_change.ok_or_else(|| {
            tracing::warn!(symbol = %symbol, "Ticker price_change_24h missing");
            ExchangeError::no_data(format!("Ticker price_change_24h missing for {}", symbol))
        })?;
        let price_change_pct_24h = t.price_change_pct.ok_or_else(|| {
            tracing::warn!(symbol = %symbol, "Ticker price_change_pct_24h missing");
            ExchangeError::no_data(format!("Ticker price_change_pct_24h missing for {}", symbol))
        })?;

        Ok(Ticker {
            symbol: t.symbol,
            exchange: t.exchange,
            bid,
            ask,
            last,
            high_24h,
            low_24h,
            volume_24h,
            price_change_24h,
            price_change_pct_24h,
            timestamp: t.timestamp.unwrap_or_else(chrono::Utc::now),
        })
    }
}

/// CCXT-internal kline (minimal, from OHLCV arrays).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CcxtKline {
    pub timestamp: i64,
    /// K线收盘时间（毫秒），来自交易所返回的 a[6] 字段。
    /// 为 None 时由下游 `to_models_kline` 计算为 `timestamp + interval_ms - 1`。
    pub close_time: Option<i64>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub quote_volume: Option<f64>,
    pub trades: Option<i64>,
}

/// CCXT-internal order book (with optional nonce).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CcxtOrderBook {
    pub symbol: String,
    pub bids: Vec<(f64, f64)>,
    pub asks: Vec<(f64, f64)>,
    pub timestamp: Option<DateTime<Utc>>,
    pub nonce: Option<u64>,
}

impl From<CcxtOrderBook> for OrderBook {
    fn from(ob: CcxtOrderBook) -> Self {
        OrderBook {
            symbol: ob.symbol,
            bids: ob.bids,
            asks: ob.asks,
            timestamp: ob.timestamp.unwrap_or_else(chrono::Utc::now),
        }
    }
}

/// CCXT-internal funding rate (with raw exchange data).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CcxtFundingRate {
    pub symbol: String,
    pub rate: f64,
    pub next_funding_time: Option<DateTime<Utc>>,
    pub info: serde_json::Value,
}

impl From<CcxtFundingRate> for FundingRate {
    fn from(fr: CcxtFundingRate) -> Self {
        FundingRate {
            symbol: fr.symbol,
            rate: fr.rate,
            next_funding_time: fr.next_funding_time,
        }
    }
}

/// CCXT-internal funding history entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CcxtFundingHistoryEntry {
    pub funding_time: DateTime<Utc>,
    pub rate: f64,
}

impl From<CcxtFundingHistoryEntry> for FundingHistoryEntry {
    fn from(e: CcxtFundingHistoryEntry) -> Self {
        FundingHistoryEntry {
            funding_time: e.funding_time,
            rate: e.rate,
        }
    }
}

/// API key restrictions info (from /sapi/v1/account/apiRestrictions).
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
