use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AutoBot {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub symbol: String,
    pub exchange: String,
    pub market_type: String,
    pub paper_mode: bool,
    pub status: String,
    pub leverage: i32,
    pub max_position_pct: f64,
    pub decide_interval_secs: i32,
    pub position_id: Option<Uuid>,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub market_regime: Option<String>,
    pub ai_analysis: Option<String>,
    pub system_prompt: Option<String>,
    pub user_prompt: Option<String>,
    pub total_pnl: f64,
    pub total_trades: i32,
    pub win_trades: i32,
    pub loss_trades: i32,
    pub last_decided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub stopped_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AutoTrade {
    pub id: Uuid,
    pub bot_id: Uuid,
    pub user_id: Uuid,
    pub symbol: String,
    pub exchange: String,
    pub side: String,
    pub trade_type: String,
    pub trigger_source: String,
    pub price: f64,
    pub quantity: f64,
    pub pnl: f64,
    pub pnl_pct: f64,
    pub exchange_order_id: Option<String>,
    pub created_at: DateTime<Utc>,
}
