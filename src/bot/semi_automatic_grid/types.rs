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

/// Default user prompt template for AI analysis
pub const DEFAULT_USER_PROMPT_TEMPLATE: &str = r#"当前时间：{timestamp}
交易对：{symbol}
总投资额：{total_investment} USDT
杠杆倍数：{leverage}

## 网格运行状态
- 状态：{grid_status}
- 上次调整时间：{last_adjust_time}
- 连续亏损配对次数：{consecutive_losses}

## 当前网格配置（若状态为 running 或 paused，否则填 "none"）
{current_grid_config}

## 当前持仓
- 持仓币数：{position_base}
- 持仓方向：{position_side}
- 开仓均价：{entry_price}
- 未实现盈亏：{unrealized_pnl} USDT
- 已用保证金：{used_margin} USDT

## 当前挂单（与交易所实际一致）
{open_orders}

## 资金费率与事件
- 当前资金费率：{funding_rate}
- 下一个结算时间：{funding_next_time}
- 重大事件标志：{event_flag}
- 事件描述（若event_flag为true）：{event_description}

## 市场数据（1小时主周期）
- 当前价格：{h1_current_price}
- 布林带上轨：{h1_bb_upper}
- 布林带中轨：{h1_bb_middle}
- 布林带下轨：{h1_bb_lower}
- 带宽百分比：{h1_bb_width_pct}%
- EMA20：{h1_ema20}
- EMA50：{h1_ema50}
- EMA距离百分比：{h1_ema_distance_pct}%
- ADX(14)：{h1_adx}
- ATR(14)：{h1_atr}
- ATR(14)的20周期均值：{h1_atr_sma20}
- 最近1h K线实体：{h1_candle_body}
- 连续出轨方向及根数：{h1_bars_outside_band}
- 5根K线前带宽：{h1_bandwidth_5bars_ago}%
- 近20根K线最高价：{h1_high_20}
- 近20根K线最低价：{h1_low_20}
- 最接近的上方整数关口：{nearest_round_up}
- 最接近的下方整数关口：{nearest_round_down}

## 市场数据（15分钟辅助周期）
- 当前价格：{m15_current_price}
- 带宽百分比：{m15_bb_width_pct}%
- ATR(14)：{m15_atr}
- ATR的20均值：{m15_atr_sma20}
- ADX(14)：{m15_adx}
- 连续出轨方向及根数：{m15_bars_outside_band}
- EMA20：{m15_ema20}
- EMA50：{m15_ema50}

## 市场数据（4小时辅助周期）
- EMA20：{h4_ema20}
- EMA50：{h4_ema50}
- ADX(14)：{h4_adx}
- 带宽百分比：{h4_bb_width_pct}%

## 触发原因
{trigger_reason}"#;

