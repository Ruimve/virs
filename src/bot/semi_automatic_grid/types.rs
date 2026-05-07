use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 单个网格层的状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridLevel {
    pub level: i32,           // 层级编号 (0 = 最低价, grid_count-1 = 最高价)
    pub price: f64,           // 该层价格
    pub buy_price: f64,       // 买入挂单价格（该层价格）
    pub sell_price: f64,      // 卖出挂单价格（下一层价格 = buy_price * (1 + profit_pct)）
    pub quantity: f64,        // 该层数量
    pub buy_order_id: Option<Uuid>,   // 买入订单 ID（Position Engine 内部）
    pub sell_order_id: Option<Uuid>,  // 卖出订单 ID
    pub buy_filled: bool,     // 买入是否已成交
    pub sell_filled: bool,    // 卖出是否已成交
    pub hold_quantity: f64,   // 当前持有数量（买入成交 - 卖出成交）
}

/// 网格运行状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridState {
    pub bot_id: Uuid,
    pub symbol: String,
    pub exchange: String,
    pub levels: Vec<GridLevel>,
    pub current_price: f64,
    pub total_pnl: f64,
    pub total_trades: i32,
    pub grid_filled_count: i32,
    pub last_tick_at: DateTime<Utc>,
}

/// 网格引擎命令
#[derive(Debug)]
pub enum GridCommand {
    StartBot { bot_id: Uuid },
    StopBot { bot_id: Uuid },
    PauseBot { bot_id: Uuid },
    ResumeBot { bot_id: Uuid },
    DeleteBot { bot_id: Uuid },
    AdjustGrid { bot_id: Uuid },
    Shutdown,
}

/// 分析日志记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisLog {
    pub bot_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub analysis_type: String,  // "initial" | "periodic"
    pub system_prompt: String,
    pub user_prompt: String,
    pub result: serde_json::Value,
    pub error: Option<String>,
}

/// 网格引擎事件（发送给前端 WebSocket）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GridEvent {
    BotStarted { bot_id: Uuid },
    BotStopped { bot_id: Uuid, reason: String },
    BotError { bot_id: Uuid, error: String },
    GridFilled { bot_id: Uuid, level: i32, side: String, price: f64, quantity: f64 },
    GridTradeClosed { bot_id: Uuid, level: i32, pnl: f64 },
    PriceUpdate { bot_id: Uuid, price: f64 },
    StatusUpdate { bot_id: Uuid, state: GridState },
    AnalysisLog { log: AnalysisLog },
}

