use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/** 单个网格层的状态

每个层级代表网格中的一个价格节点，包含买卖价格、持仓量、订单状态等 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridLevel {
/** 层级编号（从 0 开始） */
    pub level: i32,
/** 层级中心价格 */
    pub price: f64,
/** 层级方向："buy" 或 "sell" */
    pub side: String,
/** 买入挂单价格 */
    pub buy_price: f64,
/** 卖出挂单价格 */
    pub sell_price: f64,
/** 每层数量（币数） */
    pub quantity: f64,
/** 买单交易所订单 ID */
    pub buy_order_id: Option<Uuid>,
/** 卖单交易所订单 ID */
    pub sell_order_id: Option<Uuid>,
/** 买单是否已成交 */
    pub buy_filled: bool,
/** 卖单是否已成交 */
    pub sell_filled: bool,
/** 当前持仓量（正数为多头，负数为空头） */
    pub hold_quantity: f64,
/** 加权平均买入价（用于 PnL 计算） */
    pub avg_buy_price: f64,
/** 最近一次成交价格 */
    pub last_fill_price: Option<f64>,
}

impl GridLevel {
/** 创建重置后的层级副本，用于买卖周期完成后重新挂单

保留层级结构参数（价格、方向、数量），清除所有运行时状态 */
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
        }
    }
}

/** 网格运行状态快照

用于向前端 WebSocket 推送当前 bot 的完整运行状态 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridState {
    pub bot_id: Uuid,
    pub symbol: String,
    pub exchange: String,
/** 所有网格层级状态 */
    pub levels: Vec<GridLevel>,
/** 当前市场价格 */
    pub current_price: f64,
/** 累计已实现盈亏 */
    pub total_pnl: f64,
/** 未实现浮动盈亏 */
    pub unrealized_pnl: f64,
/** 总成交次数 */
    pub total_trades: i32,
/** 买卖配对完成次数 */
    pub grid_filled_count: i32,
/** 最后一次 tick 时间 */
    pub last_tick_at: DateTime<Utc>,
}

/** 网格引擎命令

外部通过 mpsc channel 发送给 GridEngine 的控制命令 */
#[derive(Debug)]
pub enum GridCommand {
/** 启动指定 bot */
    StartBot { bot_id: Uuid },
/** 停止指定 bot */
    StopBot { bot_id: Uuid },
/** 暂停指定 bot（取消挂单但保留状态） */
    PauseBot { bot_id: Uuid },
/** 恢复指定 bot */
    ResumeBot { bot_id: Uuid },
/** 删除指定 bot（可选平仓） */
    DeleteBot { bot_id: Uuid, close_position: bool },
/** 触发指定 bot 重新加载配置并调整网格 */
    AdjustGrid { bot_id: Uuid },
/** 关闭整个引擎 */
    Shutdown,
}

/** AI 分析日志记录

记录每次 LLM 调用的输入输出，用于审计和回溯 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisLog {
    pub bot_id: Uuid,
    pub timestamp: DateTime<Utc>,
/** 分析类型："initial" 或 "periodic" */
    pub analysis_type: String,
/** 发送给 LLM 的系统提示词 */
    pub system_prompt: String,
/** 发送给 LLM 的用户提示词 */
    pub user_prompt: String,
/** LLM 返回的 JSON 结果 */
    pub result: serde_json::Value,
/** 错误信息（LLM 调用失败时记录） */
    pub error: Option<String>,
}

/** 网格引擎事件

通过 broadcast channel 推送给前端 WebSocket，用于实时状态展示 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GridEvent {
/** Bot 启动 */
    BotStarted { bot_id: Uuid },
/** Bot 停止 */
    BotStopped { bot_id: Uuid, reason: String },
/** Bot 运行错误 */
    BotError { bot_id: Uuid, error: String },
/** 网格参数调整完成 */
    GridAdjusted { bot_id: Uuid, upper_price: f64, lower_price: f64, level_count: usize },
/** 网格层成交 */
    GridFilled { bot_id: Uuid, level: i32, side: String, price: f64, quantity: f64 },
/** 买卖配对完成（一整个网格周期结束） */
    GridTradeClosed { bot_id: Uuid, level: i32, pnl: f64 },
/** 价格更新 */
    PriceUpdate { bot_id: Uuid, price: f64 },
/** 完整状态更新推送 */
    StatusUpdate { bot_id: Uuid, state: GridState },
/** AI 分析日志 */
    AnalysisLog { log: AnalysisLog },
}

/** 默认用户 Prompt 模板

包含所有占位符 {xxx}，由 utils::prompt::render_prompt 渲染替换 */
pub const DEFAULT_USER_PROMPT_TEMPLATE: &str = r#"当前时间：{timestamp}
交易对：{symbol}

## 账户资产
- 总资产：{total_balance} USDT
- 可用余额：{available_balance} USDT
- 已用保证金：{used_margin} USDT
- 保证金使用率：{margin_usage_rate}%
- 杠杆倍数：{leverage}

## 网格运行状态
- 状态：{grid_status}
- 上次调整时间：{last_adjust_time}
- 连续亏损配对次数：{consecutive_losses}

## 当前网格配置（若状态为 running 或 paused，否则填 "none"）
{current_grid_config}

## 当前持仓
{position_info}

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
