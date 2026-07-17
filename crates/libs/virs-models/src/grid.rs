use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::StrategyStatus;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GridBot {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub symbol: String,
    pub exchange: String,
    pub paper_mode: bool,
    pub status: StrategyStatus,
    pub upper_price: f64,
    pub lower_price: f64,
    pub grid_count: i32,
    pub grid_profit_pct: f64,
    pub quantity_per_grid: f64,
    pub leverage: i32,
    pub initial_capital: f64,
    pub market_regime: Option<String>,
    pub ai_analysis: Option<String>,
    pub grid_levels_json: Option<serde_json::Value>,
    pub system_prompt: Option<String>,
    pub user_prompt: Option<String>,
    pub dynamic_adjust: bool,
    pub adjust_interval_secs: i32,
    pub last_adjusted_at: Option<DateTime<Utc>>,
    pub total_pnl: f64,
    pub unrealized_pnl: f64,
    pub total_trades: i32,
    pub grid_filled_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub stopped_at: Option<DateTime<Utc>>,
}

impl GridBot {
    pub fn grid_spacing(&self) -> f64 {
        if self.grid_count <= 0 {
            0.0
        } else {
            (self.upper_price - self.lower_price) / self.grid_count as f64
        }
    }

    pub fn is_running(&self) -> bool {
        self.status == StrategyStatus::Running
    }

    pub fn is_stopped(&self) -> bool {
        self.status == StrategyStatus::Stopped
    }

    pub fn total_return_pct(&self) -> f64 {
        if self.initial_capital == 0.0 {
            0.0
        } else {
            self.total_pnl / self.initial_capital * 100.0
        }
    }
}
