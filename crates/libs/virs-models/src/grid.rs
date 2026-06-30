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
    pub market_type: String,
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
    /// Calculate the price spacing between grid levels.
    /// Returns 0.0 if grid_count is zero (division-by-zero protection).
    pub fn grid_spacing(&self) -> f64 {
        if self.grid_count <= 0 {
            0.0
        } else {
            (self.upper_price - self.lower_price) / self.grid_count as f64
        }
    }

    /// Validate grid configuration parameters.
    /// Returns true only if grid_count > 0 and upper_price > lower_price.
    pub fn is_valid_config(&self) -> bool {
        self.grid_count > 0 && self.upper_price > self.lower_price
    }

    /// Returns true if the bot is currently running.
    pub fn is_running(&self) -> bool {
        self.status == StrategyStatus::Running
    }

    /// Returns true if the bot has been stopped.
    pub fn is_stopped(&self) -> bool {
        self.status == StrategyStatus::Stopped
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
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GridTrade {
    pub id: Uuid,
    pub bot_id: Uuid,
    pub user_id: Uuid,
    pub symbol: String,
    pub exchange: String,
    pub grid_level: i32,
    pub open_side: String,
    pub open_price: f64,
    pub open_quantity: f64,
    pub open_order_id: Option<String>,
    pub opened_at: DateTime<Utc>,
    pub close_side: Option<String>,
    pub close_price: Option<f64>,
    pub close_quantity: Option<f64>,
    pub close_order_id: Option<String>,
    pub closed_at: Option<DateTime<Utc>>,
    pub pnl: f64,
    pub pnl_pct: f64,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

impl GridTrade {
    /// Returns true if the trade is still open (status == "open").
    pub fn is_open(&self) -> bool {
        self.status == "open"
    }

    /// Returns true if the trade has been closed (status == "closed").
    pub fn is_closed(&self) -> bool {
        self.status == "closed"
    }
}
