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
