use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use sqlx::FromRow;

// ============================================================
// Market Data Models
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum MarketType {
    Spot,
    Perpetual,
}

impl std::fmt::Display for MarketType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MarketType::Spot => write!(f, "spot"),
            MarketType::Perpetual => write!(f, "perpetual"),
        }
    }
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBook {
    pub symbol: String,
    pub bids: Vec<(f64, f64)>, // (price, quantity)
    pub asks: Vec<(f64, f64)>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    pub asset: String,
    pub free: f64,
    pub used: f64,
    pub total: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangePosition {
    pub symbol: String,
    pub side: PositionSide,
    pub size: f64,
    pub entry_price: f64,
    pub leverage: u32,
    pub unrealized_pnl: f64,
    pub liquidation_price: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingRate {
    pub symbol: String,
    pub rate: f64,
    pub next_funding_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingHistoryEntry {
    pub funding_time: i64,
    pub rate: f64,
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

// ============================================================
// Strategy Models
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum StrategyStatus {
    Draft,
    Running,
    Paused,
    Stopped,
    Error,
}

// ============================================================
// Position Models (shared with exchange module)
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum PositionSide {
    Long,
    Short,
}

// ============================================================
// User Models
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    Manager,
    User,
    Viewer,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub password_hash: String,
    pub role: UserRole,
    pub email: Option<String>,
    pub is_active: bool,
    pub credits: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub username: String,
    pub role: UserRole,
    pub email: Option<String>,
    pub is_active: bool,
    pub credits: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserResponse,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub email: Option<String>,
    pub role: Option<UserRole>,
}

// ============================================================
// API Common Models
// ============================================================

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

// ============================================================
// Grid Bot Models
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GridBot {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub symbol: String,
    pub exchange: String,
    pub status: StrategyStatus,
    pub upper_price: f64,
    pub lower_price: f64,
    pub grid_count: i32,
    pub grid_profit_pct: f64,
    pub quantity_per_grid: f64,
    pub leverage: i32,
    pub market_regime: Option<String>,
    pub ai_analysis: Option<String>,
    pub system_prompt: Option<String>,
    pub user_prompt: Option<String>,
    pub dynamic_adjust: bool,
    pub adjust_interval_secs: i32,
    pub last_adjusted_at: Option<DateTime<Utc>>,
    pub total_pnl: f64,
    pub total_trades: i32,
    pub grid_filled_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub stopped_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GridTrade {
    pub id: Uuid,
    pub bot_id: Uuid,
    pub user_id: Uuid,
    pub symbol: String,
    pub exchange: String,
    pub side: String,
    pub grid_level: i32,
    pub price: f64,
    pub quantity: f64,
    pub pnl: f64,
    pub pnl_pct: f64,
    pub order_id: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}
