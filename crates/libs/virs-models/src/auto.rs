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

impl AutoBot {
    /// Calculate win rate as a percentage (win_trades / total_trades * 100).
    /// Returns 0.0 if total_trades is zero (division-by-zero protection).
    pub fn win_rate(&self) -> f64 {
        if self.total_trades <= 0 {
            0.0
        } else {
            self.win_trades as f64 / self.total_trades as f64 * 100.0
        }
    }

    /// Calculate loss rate as a percentage (loss_trades / total_trades * 100).
    /// Returns 0.0 if total_trades is zero (division-by-zero protection).
    pub fn loss_rate(&self) -> f64 {
        if self.total_trades <= 0 {
            0.0
        } else {
            self.loss_trades as f64 / self.total_trades as f64 * 100.0
        }
    }

    /// Calculate total return as a percentage of initial capital.
    /// Returns 0.0 if initial_capital is zero (division-by-zero protection).
    pub fn total_return_pct(&self) -> f64 {
        if self.initial_capital == 0.0 {
            0.0
        } else {
            self.total_pnl / self.initial_capital * 100.0
        }
    }

    /// Returns true if the bot is currently running (status == "running").
    pub fn is_running(&self) -> bool {
        self.status == "running"
    }

    /// Returns true if the bot has been stopped (status == "stopped").
    pub fn is_stopped(&self) -> bool {
        self.status == "stopped"
    }
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

impl AutoTrade {
    /// Returns true if the trade is still open (status == "open").
    pub fn is_open(&self) -> bool {
        self.status == "open"
    }

    /// Returns true if the trade has been closed (status == "closed").
    pub fn is_closed(&self) -> bool {
        self.status == "closed"
    }
}
