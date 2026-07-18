use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use virs_error::ExchangeError;

// 从 virs-types 重导出 (CcxtOrder 等类型已移至 virs-types 避免循环依赖)
pub use virs_types::{CcxtOrder, CcxtOrderStatus, ExecutionType, OrderResult};

pub use virs_types::enums::{MarketType, OrderStatus, OrderType, PositionMode, PositionSide, Side};
pub use virs_types::market::{
    Balance, ExchangePosition, FundingRate, Kline, OrderBook, Ticker,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketInfo {
    pub id: String,
    pub symbol: String,
    pub base: String,
    pub quote: String,
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
    pub leverage: Option<u32>,
    pub margin_mode: Option<MarginMode>,
    pub position_side: Option<PositionSide>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum MarginMode {
    Cross,
    Isolated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderFee {
    pub cost: f64,
    pub currency: String,
    pub rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TimeInForce {
    Gtc,
    Ioc,
    Fok,
    Poc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub side: PositionSide,
    pub quantity: f64,
    pub entry_price: f64,
    pub margin_mode: MarginMode,
    pub info: serde_json::Value,
}

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
    pub price_change: Option<f64>,
    pub price_change_pct: Option<f64>,
    pub timestamp: Option<DateTime<Utc>>,
    pub info: serde_json::Value,
}

impl TryFrom<CcxtTicker> for Ticker {
    type Error = ExchangeError;

    fn try_from(t: CcxtTicker) -> Result<Self, Self::Error> {
        let symbol = t.symbol.clone();

        let bid = t.bid;
        let ask = t.ask;

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
            ExchangeError::no_data(format!(
                "Ticker price_change_pct_24h missing for {}",
                symbol
            ))
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CcxtKline {
    pub timestamp: i64,

    pub close_time: Option<i64>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub quote_volume: Option<f64>,
    pub trades: Option<i64>,
}

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
