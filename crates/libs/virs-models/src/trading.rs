//! Application-level trading types.
//!
//! These types represent the API-layer view of orders, positions, etc.
//! They differ from the Position Engine's internal types (PositionOrder, etc.)
//! by being simpler and focused on API responses.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use virs_types::enums::*;

/// API-level order representation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    pub fee: f64,
    pub fee_currency: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Order {
    /// Returns true if the order is fully filled.
    pub fn is_filled(&self) -> bool {
        self.status == OrderStatus::Filled
    }

    /// Returns true if the order is still open (Open or PartiallyFilled).
    pub fn is_open(&self) -> bool {
        matches!(self.status, OrderStatus::Open | OrderStatus::PartiallyFilled)
    }

    /// Returns true if the order was canceled.
    pub fn is_canceled(&self) -> bool {
        self.status == OrderStatus::Canceled
    }

    /// Returns the fill rate as a ratio (filled / amount).
    /// Returns 0.0 if amount is zero (division-by-zero protection).
    pub fn fill_rate(&self) -> f64 {
        if self.amount == 0.0 {
            0.0
        } else {
            self.filled / self.amount
        }
    }

    /// Returns true if this is a buy order.
    pub fn is_buy(&self) -> bool {
        self.side == Side::Buy
    }

    /// Returns true if this is a sell order.
    pub fn is_sell(&self) -> bool {
        self.side == Side::Sell
    }
}
