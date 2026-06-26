use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

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
    pub initial_capital: f64,
    pub position_id: Option<Uuid>,
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
    // 开仓
    pub open_side: String,
    pub open_price: f64,
    pub open_quantity: f64,
    pub open_order_id: Option<String>,
    pub open_fee: f64,
    pub opened_at: DateTime<Utc>,
    // 平仓（未平仓时为 NULL）
    pub close_side: Option<String>,
    pub close_price: Option<f64>,
    pub close_quantity: Option<f64>,
    pub close_order_id: Option<String>,
    pub close_fee: f64,
    pub closed_at: Option<DateTime<Utc>>,
    // 盈亏
    pub pnl: f64,
    pub pnl_pct: f64,
    // 触发源与平仓原因
    pub trigger_source: String,
    pub close_reason: Option<String>,
    // 状态
    pub status: String,
    pub created_at: DateTime<Utc>,
}
