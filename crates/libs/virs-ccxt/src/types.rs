use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use virs_error::ExchangeError;

// 本文件定义 ccxt 适配层的本地类型。virs_type 中的类型通过 `use` 直接引入，
// 不再从此处重导出 —— 调用方应直接使用 `virs_type::...`。
use virs_type::{
    market::{FundingRate, OrderBook, Ticker},
    MarginMode, MarketType, OrderType, PositionSide, Side, TimeInForce,
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

/// ccxt 层下单参数。包含 adapter 层独有的字段（market_type/leverage/margin_mode），
/// 这些字段由 CcxtAdapter 从 virs_type::PlaceOrderParams 构造时填充。
#[derive(Debug, Clone)]
pub struct CcxtPlaceOrderParams {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderFee {
    pub cost: f64,
    pub currency: String,
    pub rate: Option<f64>,
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
            tracing::error!(symbol = %symbol, "Ticker high_24h missing");
            ExchangeError::no_data(format!("Ticker high_24h missing for {}", symbol))
        })?;
        let low_24h = t.low.ok_or_else(|| {
            tracing::error!(symbol = %symbol, "Ticker low_24h missing");
            ExchangeError::no_data(format!("Ticker low_24h missing for {}", symbol))
        })?;
        let volume_24h = t.volume.ok_or_else(|| {
            tracing::error!(symbol = %symbol, "Ticker volume_24h missing");
            ExchangeError::no_data(format!("Ticker volume_24h missing for {}", symbol))
        })?;
        let price_change_24h = t.price_change.ok_or_else(|| {
            tracing::error!(symbol = %symbol, "Ticker price_change_24h missing");
            ExchangeError::no_data(format!("Ticker price_change_24h missing for {}", symbol))
        })?;
        let price_change_pct_24h = t.price_change_pct.ok_or_else(|| {
            tracing::error!(symbol = %symbol, "Ticker price_change_pct_24h missing");
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
            timestamp: t.timestamp.ok_or_else(|| {
                tracing::error!(symbol = %symbol, "Ticker timestamp missing");
                ExchangeError::no_data(format!("Ticker timestamp missing for {}", symbol))
            })?,
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

impl TryFrom<CcxtOrderBook> for OrderBook {
    type Error = ExchangeError;

    fn try_from(ob: CcxtOrderBook) -> Result<Self, Self::Error> {
        let symbol = ob.symbol.clone();
        let timestamp = ob.timestamp.ok_or_else(|| {
            tracing::warn!(symbol = %symbol, "OrderBook timestamp missing");
            ExchangeError::no_data(format!("OrderBook timestamp missing for {}", symbol))
        })?;
        Ok(OrderBook {
            symbol: ob.symbol,
            bids: ob.bids,
            asks: ob.asks,
            timestamp,
        })
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
