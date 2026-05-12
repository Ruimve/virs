use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use sqlx::FromRow;

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
    pub grid_levels_json: Option<String>,
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
