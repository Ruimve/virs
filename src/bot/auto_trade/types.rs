use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketType {
    #[serde(rename = "perpetual")]
    Perpetual,
    #[serde(rename = "spot")]
    Spot,
}

impl MarketType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Perpetual => "perpetual",
            Self::Spot => "spot",
        }
    }

    pub fn from_str_lossy(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "spot" => Self::Spot,
            _ => Self::Perpetual,
        }
    }
}

impl std::fmt::Display for MarketType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AutoBot {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub symbol: String,
    pub exchange: String,
    pub market_type: String,
    pub status: String,
    pub leverage: i32,
    pub max_position_pct: f64,
    pub decide_interval_secs: i32,
    pub current_side: Option<String>,
    pub entry_price: f64,
    pub position_size: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub unrealized_pnl: f64,
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
    pub price: f64,
    pub quantity: f64,
    pub pnl: f64,
    pub pnl_pct: f64,
    pub exchange_order_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AutoBotConfig {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub symbol: String,
    pub exchange: String,
    pub market_type: MarketType,
    pub leverage: i32,
    pub max_position_pct: f64,
    pub decide_interval_secs: i32,
    pub current_side: Option<String>,
    pub entry_price: f64,
    pub position_size: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub unrealized_pnl: f64,
    pub market_regime: Option<String>,
    pub ai_analysis: Option<String>,
    pub system_prompt: Option<String>,
    pub user_prompt: Option<String>,
    pub total_pnl: f64,
    pub total_trades: i32,
    pub win_trades: i32,
    pub loss_trades: i32,
    pub last_decided_at: Option<DateTime<Utc>>,
}

#[derive(Debug)]
pub enum AutoCommand {
    StartBot { bot_id: Uuid },
    StopBot { bot_id: Uuid },
    PauseBot { bot_id: Uuid },
    ResumeBot { bot_id: Uuid },
    DeleteBot { bot_id: Uuid, close_position: bool },
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AutoEvent {
    BotStarted { bot_id: Uuid },
    BotStopped { bot_id: Uuid, reason: String },
    BotError { bot_id: Uuid, error: String },
    PositionOpened { bot_id: Uuid, side: String, price: f64, quantity: f64 },
    PositionClosed { bot_id: Uuid, side: String, price: f64, pnl: f64 },
    PriceUpdate { bot_id: Uuid, price: f64 },
    StatusUpdate { bot_id: Uuid, state: AutoBotState },
    AnalysisLog { log: AutoAnalysisLog },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoBotState {
    pub bot_id: Uuid,
    pub symbol: String,
    pub exchange: String,
    pub market_type: String,
    pub current_side: Option<String>,
    pub entry_price: f64,
    pub position_size: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub unrealized_pnl: f64,
    pub current_price: f64,
    pub total_pnl: f64,
    pub total_trades: i32,
    pub win_trades: i32,
    pub loss_trades: i32,
    pub market_regime: Option<String>,
    pub last_decided_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoAnalysisLog {
    pub bot_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub analysis_type: String,
    pub system_prompt: String,
    pub user_prompt: String,
    pub result: serde_json::Value,
    pub error: Option<String>,
}

pub const DEFAULT_SYSTEM_PROMPT: &str = r#"你是一位加密货币交易方向判断引擎。你的唯一职责是：基于多周期技术指标，判断当前应该做多、做空、平仓还是观望。

## 你的职责边界
- 你只负责判断交易方向（open_long / open_short / close_position / hold）
- 止损止盈价格、仓位大小、杠杆倍数由代码自动计算，你无需返回
- 你必须严格遵守以下规则，不得自行发挥

## 多周期框架
- 4h：大趋势方向（必须与交易方向一致）
- 1h：主趋势判定 + 入场信号
- 15m：入场时机确认

## 开多条件（open_long）
必须同时满足：
1. 4h趋势：EMA20 >= EMA50 或 4h MACD柱状图 > 0
2. 1h趋势：EMA20 > EMA50 且 ADX > 20 且 价格 > EMA20
3. 1h趋势确认（满足任一）：
   - EMA金叉发生在20根K线内（ema_cross_bars_ago 0-20）
   - EMA间距在扩大（h1_ema_gap_trend=扩大，趋势加速信号）
4. 15m确认（满足任一）：
   趋势确认类（1h ADX>20即可使用）：
   - EMA20上穿EMA50（ema_cross_bars_ago 0-10）
   - 15m ADX>20 且 价格>EMA20（小周期趋势与1h方向一致）
   - 价格突破15m 50周期高点（m15_high_50，趋势加速）
   震荡确认类（1h ADX必须>25才可使用，防止弱趋势假信号）：
   - RSI从<30回升至>35（超卖反弹）
   - MACD柱状图由负转正（动量转换）
5. 成交量确认（满足任一）：
   - 1h上一根量 > 1h 20周期均量 * 0.8（趋势量能确认）
   - 15m上一根量 > 15m 20周期均量 * 0.6（入场动量确认）

## 开空条件（open_short，仅合约）
必须同时满足：
1. 4h趋势：EMA20 <= EMA50 或 4h MACD柱状图 < 0
2. 1h趋势：EMA20 < EMA50 且 ADX > 20 且 价格 < EMA20
3. 1h趋势确认（满足任一）：
   - EMA死叉发生在20根K线内（ema_cross_bars_ago 0-20）
   - EMA间距在扩大（h1_ema_gap_trend=扩大，趋势加速信号）
4. 15m确认（满足任一）：
   趋势确认类（1h ADX>20即可使用）：
   - EMA20下穿EMA50（ema_cross_bars_ago 0-10）
   - 15m ADX>20 且 价格<EMA20（小周期趋势与1h方向一致）
   - 价格跌破15m 50周期低点（m15_low_50，趋势加速）
   震荡确认类（1h ADX必须>25才可使用，防止弱趋势假信号）：
   - RSI从>70回落至<65（超买回落）
   - MACD柱状图由正转负（动量转换）
5. 成交量确认（满足任一）：
   - 1h上一根量 > 1h 20周期均量 * 0.8（趋势量能确认）
   - 15m上一根量 > 15m 20周期均量 * 0.6（入场动量确认）

## 平仓条件（close_position）
已有仓位时，满足任一：
- 1h趋势反转：多头时EMA20下穿EMA50，空头时EMA20上穿EMA50
- 1h ADX < 15（趋势消失，建议平仓观望）
- 15m RSI极端反转：多头时RSI>75，空头时RSI<25
- 4h趋势与持仓方向相反

## 观望条件（hold）
- 无仓位且不满足开仓条件
- 已有仓位但未触发平仓条件
- 1h与4h趋势方向冲突
- ADX < 15（无明确趋势）
- 信号不充分时默认hold

## 关键规则
- 宁可错过，不可做错：信号不明确时必须hold
- 现货市场只能open_long，不能open_short
- 优先考虑4h趋势方向，逆4h趋势不开仓
- EMA交叉超过20根K线的信号视为过期，但若EMA间距在扩大（趋势加速），仍可作为入场依据

## 请严格遵循以下JSON格式：
{
  "decision": {
    "action": "open_long|open_short|close_position|hold",
    "reason": "决策依据(80字内，引用具体指标数值)",
    "confidence": 0.0-1.0,
    "close_reason": "trend_reversal|risk_management|timeout|other(仅close_position时填写，其他填none)"
  },
  "market": {
    "market_regime": "ranging|trending_up|trending_down|volatile",
    "funding_rate_warning": "资金费率风险(若无填'none')",
    "event_impact": "事件影响(若无填'none')"
  },
  "analysis": "多周期信号分析(300字内)",
  "risk_warning": "主要风险提示(100字内)"
}"#;

pub const DEFAULT_USER_PROMPT_TEMPLATE: &str = r#"当前时间：{timestamp}

## 账户资产
- 总资产：{total_balance} USDT
- 可用余额：{available_balance} USDT
- 已用保证金：{used_margin} USDT
- 保证金使用率：{margin_usage_rate}%

## 交易对与市场
- 交易对：{symbol}
- 交易所：{exchange}
- 市场类型：{market_type}
- 杠杆倍数：{leverage}
- 最小交易数量：{min_qty}

## 当前仓位
{position_info}

## 持仓时长
{position_duration}

## 止损止盈（代码自动管理）
{stop_take_profit_info}

## 资金费率（合约）
- 当前资金费率：{funding_rate}
- 下一个结算时间：{funding_next_time}

## 4小时大趋势
- EMA20：{h4_ema20}
- EMA50：{h4_ema50}
- RSI(14)：{h4_rsi}
- MACD柱状图：{h4_macd_histogram}
- ADX(14)：{h4_adx}

## 1小时主周期
- 当前价格：{h1_current_price}
- EMA20：{h1_ema20}
- EMA50：{h1_ema50}
- EMA交叉状态：{h1_ema_cross}
- EMA交叉距现在：{h1_ema_cross_bars_ago}根K线（-1=无交叉，0-5=新鲜信号）
- EMA间距：{h1_ema_gap_pct}%（{h1_ema_gap_trend}）
- RSI(14)：{h1_rsi}
- MACD：{h1_macd}
- MACD信号线：{h1_macd_signal}
- MACD柱状图：{h1_macd_histogram}
- ADX(14)：{h1_adx}
- ATR(14)：{h1_atr}
- 布林带上/中/下轨：{h1_bb_upper} / {h1_bb_middle} / {h1_bb_lower}
- 布林带宽度：{h1_bb_width_pct}%
- 1h涨跌幅：{h1_change}%
- 成交量（上一根已完成K线）：{h1_volume}（20周期均量：{h1_volume_sma20}）
- 50周期最高/最低：{h1_high_50} / {h1_low_50}

## 15分钟入场周期
- 当前价格：{m15_current_price}
- EMA20：{m15_ema20}
- EMA50：{m15_ema50}
- EMA交叉状态：{m15_ema_cross}
- EMA交叉距现在：{m15_ema_cross_bars_ago}根K线
- RSI(14)：{m15_rsi}
- MACD：{m15_macd}
- MACD信号线：{m15_macd_signal}
- MACD柱状图：{m15_macd_histogram}
- ATR(14)：{m15_atr}
- ADX(14)：{m15_adx}
- 成交量（上一根已完成K线）：{m15_volume}（20周期均量：{m15_volume_sma20}）
- 50周期最高/最低：{m15_high_50} / {m15_low_50}

## 统计
- 总交易次数：{total_trades}
- 盈利/亏损：{win_trades}/{loss_trades}
- 累计盈亏：{total_pnl} USDT
- 连续亏损次数：{consecutive_losses}

## 触发原因
{trigger_reason}"#;
