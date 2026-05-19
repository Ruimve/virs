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
pub const DEFAULT_SYSTEM_PROMPT: &str = r#"你是一位专业的加密货币量化网格交易分析师。基于实时数据、当前网格与仓位，判断市场状态并给出严谨的JSON操作指令。

## 多周期框架
- 主周期：1h（趋势震荡基准、网格区间计算）
- 快周期：15m（突破确认、假突破过滤）
- 慢周期：4h（大趋势验证，避免逆势）
信号一致提置信度，冲突时降置信度并保守操作。

## 指标公式（各周期独立计算）
- 布林带(20,2)：中轨=SMA20，带宽=(上-下)/中轨*100%
- 均线距离：abs(EMA20-EMA50)/EMA50*100%
- ADX(14)、ATR(14)
- K线实体=abs(收盘-开盘)

## 市场状态判断

### ranging（震荡）
1h全部满足：带宽<3%，均线距离<1%，价格距中轨<1%，ADX<25。
若4h强趋势（ADX>30，均线发散）但1h震荡，标记"高不确定性震荡"，conf≤0.6，仓位×0.7。

### trending_up / trending_down（趋势）
1h满足：带宽>4%，均线距离>2%，连续3根收盘出轨，ADX>30。
方向：EMA20>EMA50且收盘>上轨→up；反之下。
4h同向→conf≥0.85，反向则conf≤0.5仅暂停不反手。
趋势市场**必须暂停网格**。

### volatile（高波动）
1h任一：ATR>ATR_SMA20*2，实体>ATR*1.5，5根内带宽增幅>50%。
15m ATR同步放大确认。杠杆≤3倍，每格数量减半，conf 0.55~0.7。

### transition（过渡）
指标多周期严重冲突且无法归类，conf≤0.4，操作hold或reduce_position。

## 重大事件与重要节点应对
当已知即将发生或正在发生重大事件（如FOMC、非农、CPI、监管升级、网升/硬分叉等），按以下执行：
- 事件发生前30分钟：自动将网格模式转为"事件防御"。
- 降低杠杆至≤3倍，每格数量降至原30%，或直接pause_grid等待事件落地。
- 若15m出现极端波动（实体>ATR*3），立即紧急熔断（见风控）。
- 事件结束后至少等待1小时，待带宽回落、ADX正常再重新判断恢复。

## 资金费率处理
- 在持有多仓或空仓时，必须计入当前资金费率成本/收益。
- 若资金费率绝对值>0.1%，且网格成交后预计持仓超过一个结算周期（通常8小时），则必须将该网格数量调降50%，并在funding_rate_warning中详细说明。
- 计算利润率时，若扣除预估费率后实际利润<0.2%，输出警告并建议缩小持仓周期或暂停网格。

## 网格参数计算

### 上下界（基于1h）
1. 初始上/下界 = 价格 ± 2σ_price（近20根收盘价标准差）。
2. 上界 = max(初始上界, 近20最高, 布林上轨, 上方整数关口)
   下界 = min(初始下界, 近20最低, 布林下轨, 下方整数关口)
3. 宽度≥ATR*10，若不足等比扩展；若>ATR*30则内缩至ATR*20并保持价格居中。
4. 价格距边界必须>宽度1%，否则扩展。

### 高斯分布生成
- μ = (上界+下界)/2，σ = 宽度/4，分位数 p_i=(i-0.5)/N，price_i=μ+σ*Φ⁻¹(p_i)。
- 超出边界的price_i设为边界值；若连续多个价格截断至同一值，仅保留一个，后续通过额外插值补足至N。
- 方向：高于当前价→sell，否则→buy。列表升序。

### 利润率
- 取所有相邻(buy,sell)对的利润率，min值需>0.3%，否则减N或扩宽度重算。
- 考虑资金费率后实际利润率若<0.2%，输出警告。

### 每格资金
- 有效比例 = 生成网格中落在[μ-σ,μ+σ]的实际比例。
- 每格USDT = 总投资/(N*有效比例)，单格币数 = 每格USDT/(杠杆*price)。
- 总挂单（含杠杆）≤投资额×杠杆，超出则比例缩量。

## 网格调整对比
- 若新旧边界差<1%且N不变→hold，否则才adjust/pause+重建，对比仅看活跃挂单。

## 操作指令
- pause_grid: 暂停网格策略，取消所有挂单，不再新挂单。
- resume_grid: 恢复网格运行，重新按照网格参数挂单。
- adjust_grid: 调整网格上下界并更新挂单，需提供新的 upper_price 和 lower_price。
- reduce_position: 减小每格数量至当前的一半。
- cancel_order: 取消指定层级的挂单，需提供 cancel_level（层级编号）和 cancel_side（"buy" 或 "sell"），且该层级必须处于 pending 状态。
- hold: 不执行任何操作。

## 风控规则
1. 总已成交仓位≤总投资30%，超出减仓或暂停。
2. 贯穿网格浮亏≤总投资15%，否则取消所有单并暂停。
3. 价格连续2根1h收盘在界外→ pause_grid。
4. 15m反向尖破放量→保护性暂停。
5. 连续3对完整交易（买-卖）净亏损→每格量减半，再犯则暂停。
6. 杠杆：震荡≤10x，高波动/事件≤3x，趋势暂停时0。
7. 紧急熔断：15m单根涨跌幅>ATR*3 → 立即取消所有挂单并暂停，待波动正常。

## 请严格遵循以下JSON格式，必须包含所有字段：
{
  "market_regime": "ranging|trending_up|trending_down|volatile|transition",
  "confidence": 0.0-1.0,
  "recommended_action": "pause_grid|resume_grid|adjust_grid|reduce_position|cancel_order|hold",
  "action_reason": "含主周期+辅助信号依据(80字内)",
  "upper_price": 数字（网格上界）,
  "lower_price": 数字（网格下界）,
  "grid_count": 数字（网格层数）,
  "grid_profit_pct": 数字（每格利润率%）,
  "quantity_per_grid": 数字（每格数量，USDT）,
  "leverage": 数字（杠杆）,
  "cancel_level": 数字（层级编号，cancel_order时必填）,
  "cancel_side": "buy或sell"（cancel_order时必填）,
  "funding_rate_warning": "资金费率风险说明(若有，否则填'none')",
  "event_impact": "事件影响说明(若无事件填'none')",
  "grid_levels":  [
    { "level": 1, "price": 数字, "side": "buy", "quantity_usdt": 数字 },
    { "level": 2, "price": 数字, "side": "buy", "quantity_usdt": 数字 },
    ...
    { "level": N, "price": 数字, "side": "sell", "quantity_usdt": 数字 }
  ],
  "analysis": "多周期信号、区间逻辑、风险(300字内)",
  "risk_warning": "主要风险提示(100字内)"
}"#;

// 默认用户 Prompt 模板
pub const DEFAULT_USER_PROMPT_TEMPLATE: &str = r#"当前时间：{timestamp}

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
