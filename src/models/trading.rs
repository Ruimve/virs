use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OrderType {
    Market,
    Limit,
    StopMarket,
    StopLimit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OrderStatus {
    Pending,
    Open,
    PartiallyFilled,
    Filled,
    Canceled,
    Failed,
}

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
    pub fee: f64,
    pub fee_currency: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum PositionSide {
    Long,
    Short,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PositionMode {
    OneWay,
    Hedge,
}
