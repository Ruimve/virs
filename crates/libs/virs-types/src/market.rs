//! Market data types: Ticker, Kline, OrderBook, Balance, etc.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::enums::PositionSide;

/// Ticker snapshot (API-layer, includes exchange field)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ticker {
    pub symbol: String,
    pub exchange: String,
    pub bid: f64,
    pub ask: f64,
    pub last: f64,
    pub high_24h: f64,
    pub low_24h: f64,
    pub volume_24h: f64,
    pub price_change_24h: f64,
    pub price_change_pct_24h: f64,
    pub timestamp: DateTime<Utc>,
}

impl Ticker {
    /// Returns the mid price: (bid + ask) / 2.
    pub fn mid_price(&self) -> f64 {
        (self.bid + self.ask) / 2.0
    }

    /// Returns the bid-ask spread: ask - bid.
    pub fn spread(&self) -> f64 {
        self.ask - self.bid
    }
}

/// Kline / candlestick data
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

/// Order book
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderBook {
    pub symbol: String,
    pub bids: Vec<(f64, f64)>,
    pub asks: Vec<(f64, f64)>,
    pub timestamp: DateTime<Utc>,
}

impl OrderBook {
    /// Returns the best (highest) bid price, or None if empty.
    pub fn best_bid(&self) -> Option<f64> {
        self.bids.first().map(|(p, _)| *p)
    }

    /// Returns the best (lowest) ask price, or None if empty.
    pub fn best_ask(&self) -> Option<f64> {
        self.asks.first().map(|(p, _)| *p)
    }

    /// Returns the bid-ask spread, or None if either side is empty.
    pub fn spread(&self) -> Option<f64> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some(ask - bid),
            _ => None,
        }
    }

    /// Returns the mid price, or None if either side is empty.
    pub fn mid_price(&self) -> Option<f64> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some((bid + ask) / 2.0),
            _ => None,
        }
    }
}

/// Account balance
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Balance {
    pub asset: String,
    pub free: f64,
    pub used: f64,
    pub total: f64,
}

impl Balance {
    /// Computes total from free + used.
    pub fn compute_total(&self) -> f64 {
        self.free + self.used
    }
}

/// Exchange position info
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExchangePosition {
    pub symbol: String,
    pub side: PositionSide,
    pub size: f64,
    pub entry_price: f64,
    pub leverage: u32,
    pub unrealized_pnl: f64,
    pub liquidation_price: Option<f64>,
}

impl ExchangePosition {
    pub fn is_long(&self) -> bool {
        self.side.is_long()
    }

    pub fn is_short(&self) -> bool {
        self.side.is_short()
    }

    /// Computes unrealized PnL at a given current price.
    /// Long: (current - entry) * size
    /// Short: (entry - current) * size
    pub fn unrealized_pnl_at(&self, current_price: f64) -> f64 {
        match self.side {
            PositionSide::Long => (current_price - self.entry_price) * self.size,
            PositionSide::Short => (self.entry_price - current_price) * self.size,
            PositionSide::Both => 0.0,
        }
    }

    /// Computes PnL percentage at a given current price.
    /// pnl / (entry_price * size) * 100
    /// Returns 0.0 if entry_price or size is zero (division-by-zero protection).
    pub fn pnl_pct_at(&self, current_price: f64) -> f64 {
        let cost = self.entry_price * self.size;
        if cost == 0.0 {
            0.0
        } else {
            self.unrealized_pnl_at(current_price) / cost * 100.0
        }
    }
}

/// Funding rate
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FundingRate {
    pub symbol: String,
    pub rate: f64,
    pub next_funding_time: Option<DateTime<Utc>>,
}

/// Funding history entry
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FundingHistoryEntry {
    pub funding_time: i64,
    pub rate: f64,
}

/// Fee rates
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeeRates {
    pub symbol: String,
    pub maker_rate: f64,
    pub taker_rate: f64,
}

/// API response wrapper
#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub message: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            message: None,
        }
    }

    pub fn ok_with_message(data: T, message: impl Into<String>) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            message: Some(message.into()),
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.into()),
            message: None,
        }
    }
}

/// Pagination parameters
#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

impl PaginationParams {
    pub fn normalize(&self) -> (i64, i64) {
        let page = self.page.unwrap_or(1).max(1);
        let page_size = self.page_size.unwrap_or(20).clamp(1, 100);
        (page, page_size)
    }
}
