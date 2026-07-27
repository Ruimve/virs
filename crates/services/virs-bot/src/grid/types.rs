use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridLevel {
    pub level: i32,
    pub price: f64,
    pub side: String,
    pub buy_price: f64,
    pub sell_price: f64,
    pub quantity: f64,
    pub buy_order_id: Option<Uuid>,
    pub sell_order_id: Option<Uuid>,
    pub buy_filled: bool,
    pub sell_filled: bool,
    pub hold_quantity: f64,
    pub avg_buy_price: f64,
    pub last_fill_price: Option<f64>,
    pub open_client_order_id: Option<String>,
}

impl GridLevel {
    pub fn reset_for_relist(&self) -> GridLevel {
        GridLevel {
            level: self.level,
            price: self.price,
            side: self.side.clone(),
            buy_price: self.buy_price,
            sell_price: self.sell_price,
            quantity: self.quantity,
            buy_order_id: None,
            sell_order_id: None,
            buy_filled: false,
            sell_filled: false,
            hold_quantity: 0.0,
            avg_buy_price: 0.0,
            last_fill_price: None,
            open_client_order_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridState {
    pub bot_id: Uuid,
    pub symbol: String,
    pub exchange: String,
    pub levels: Vec<GridLevel>,
    pub current_price: f64,
    pub total_pnl: f64,
    pub unrealized_pnl: f64,
    pub total_trades: i32,
    pub grid_filled_count: i32,
    pub last_tick_at: DateTime<Utc>,
}

#[derive(Debug)]
pub enum GridCommand {
    StartBot { bot_id: Uuid },
    StopBot { bot_id: Uuid },
    DeleteBot { bot_id: Uuid, close_position: bool },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GridEvent {
    BotStarted {
        bot_id: Uuid,
    },
    BotStopped {
        bot_id: Uuid,
        reason: String,
    },
    BotError {
        bot_id: Uuid,
        error: String,
    },
    GridAdjusted {
        bot_id: Uuid,
        upper_price: f64,
        lower_price: f64,
        level_count: usize,
    },
    GridFilled {
        bot_id: Uuid,
        level: i32,
        side: String,
        price: f64,
        quantity: f64,
    },
    GridTradeClosed {
        bot_id: Uuid,
        level: i32,
        pnl: f64,
    },
    StatusUpdate {
        bot_id: Uuid,
        state: GridState,
    },
}

