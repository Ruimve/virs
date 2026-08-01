//! 统一 prompt 渲染器。
//!
//! [`RenderContext`] 封装渲染 prompt 所需的全部输入数据（账户、持仓、统计、指标等），
//! [`render`] 将模板中的 `{placeholder}` 占位符替换为 context 的实际值。
//!
//! 任何 bot 都使用同一个渲染器，各自构建 context。
//! 模板中未出现的占位符 replace 是 no-op，无副作用。

use virs_types::MarketIndicators;

/// 渲染 prompt 所需的全部上下文数据。
///
/// 所有字段对任何 bot 都是通用的；不适用于当前 bot 的字段填默认值即可。
#[derive(Debug, Clone)]
pub struct RenderContext {
    // ── 时间 ──
    /// 当前时间戳（格式："2026-07-19 12:00:00 UTC"）
    pub timestamp: String,

    // ── 交易对信息 ──
    /// 交易对符号（如 "BTC/USDT"）
    pub symbol: String,
    /// 交易所名称（如 "binance"）
    pub exchange: String,
    /// 最小交易数量
    pub min_qty: f64,

    // ── 账户资产 ──
    /// 总资产（USDT）
    pub total_balance: f64,
    /// 可用余额（USDT）
    pub available_balance: f64,
    /// 已用保证金（USDT）
    pub used_margin: f64,
    /// 保证金使用率（百分比，0~100）
    pub margin_usage_rate: f64,
    /// 杠杆倍数
    pub leverage: i32,

    // ── 持仓信息 ──
    /// 持仓描述（人类可读字符串，由各 bot 自行格式化）
    pub position_info: String,
    /// 持仓时长（人类可读字符串）
    pub position_duration: String,
    /// 止损止盈信息（人类可读字符串）
    pub stop_take_profit_info: String,
    /// 最近平仓事件（人类可读字符串，用于反思避免反复扫损）
    pub recent_close_info: String,
    /// 连续亏损次数
    pub consecutive_losses: i32,

    // ── 资金费率 ──
    /// 当前资金费率（原始小数，如 0.0001 = 0.01%）
    pub funding_rate: f64,
    /// 下一个结算时间
    pub funding_next_time: String,

    // ── 交易统计 ──
    /// 总交易次数
    pub total_trades: i32,
    /// 盈利交易次数
    pub win_trades: i32,
    /// 亏损交易次数
    pub loss_trades: i32,
    /// 累计盈亏（USDT）
    pub total_pnl: f64,

    // ── 事件标记 ──
    /// 是否有重大事件
    pub event_flag: bool,
    /// 事件描述
    pub event_description: String,

    // ── 触发原因 ──
    /// 本次 LLM 决策的触发原因（如 "scheduled" / "scheduled_15m"）
    pub trigger_reason: String,

    // ── 技术指标（多周期） ──
    /// 全部市场指标快照（含 1h / 15m / 4h 三周期）
    pub ind: MarketIndicators,
}

/// 将模板中的 `{placeholder}` 占位符替换为 context 的实际值。
///
/// 支持全部 75 个占位符。模板中未出现的占位符 replace 是 no-op，无副作用。
pub fn render(template: &str, ctx: &RenderContext) -> String {
    // ── 派生字段：EMA 交叉状态 ──
    let h1_ema_cross = if ctx.ind.ema20 > ctx.ind.ema50 {
        "金叉(多头)"
    } else {
        "死叉(空头)"
    };
    let m15_ema_cross = if ctx.ind.m15_ema20 > ctx.ind.m15_ema50 {
        "金叉(多头)"
    } else {
        "死叉(空头)"
    };
    // EMA 交叉距现在的 K 线根数（-1 表示无近期交叉）
    let h1_ema_cross_bars = if ctx.ind.h1_ema_cross_bars_ago >= 0 {
        ctx.ind.h1_ema_cross_bars_ago.to_string()
    } else {
        "无近期交叉".to_string()
    };
    let m15_ema_cross_bars = if ctx.ind.m15_ema_cross_bars_ago >= 0 {
        ctx.ind.m15_ema_cross_bars_ago.to_string()
    } else {
        "无近期交叉".to_string()
    };

    template
        // ── 时间 ──
        .replace("{timestamp}", &ctx.timestamp)
        // ── 交易对信息 ──
        .replace("{symbol}", &ctx.symbol)
        .replace("{exchange}", &ctx.exchange)
        .replace("{min_qty}", &format!("{:.6}", ctx.min_qty))
        // ── 账户资产 ──
        .replace("{total_balance}", &format!("{:.2}", ctx.total_balance))
        .replace("{available_balance}", &format!("{:.2}", ctx.available_balance))
        .replace("{used_margin}", &format!("{:.2}", ctx.used_margin))
        .replace("{margin_usage_rate}", &format!("{:.1}", ctx.margin_usage_rate))
        .replace("{leverage}", &ctx.leverage.to_string())
        // ── 持仓信息 ──
        .replace("{position_info}", &ctx.position_info)
        .replace("{position_duration}", &ctx.position_duration)
        .replace("{stop_take_profit_info}", &ctx.stop_take_profit_info)
        .replace("{recent_close_info}", &ctx.recent_close_info)
        .replace("{consecutive_losses}", &ctx.consecutive_losses.to_string())
        // ── 资金费率 ──
        .replace("{funding_rate}", &format!("{:.4}%", ctx.funding_rate * 100.0))
        .replace("{funding_next_time}", &ctx.funding_next_time)
        // ── 交易统计 ──
        .replace("{total_trades}", &ctx.total_trades.to_string())
        .replace("{win_trades}", &ctx.win_trades.to_string())
        .replace("{loss_trades}", &ctx.loss_trades.to_string())
        .replace("{total_pnl}", &format!("{:.2}", ctx.total_pnl))
        // ── 事件标记 ──
        .replace("{event_flag}", if ctx.event_flag { "true" } else { "false" })
        .replace("{event_description}", &ctx.event_description)
        // ── 触发原因 ──
        .replace("{trigger_reason}", &ctx.trigger_reason)
        // ── 4h 大趋势 ──
        .replace("{h4_ema20}", &format!("{:.2}", ctx.ind.h4_ema20))
        .replace("{h4_ema50}", &format!("{:.2}", ctx.ind.h4_ema50))
        .replace("{h4_rsi}", &format!("{:.2}", ctx.ind.h4_rsi))
        .replace("{h4_macd}", &format!("{:.4}", ctx.ind.h4_macd))
        .replace("{h4_macd_signal}", &format!("{:.4}", ctx.ind.h4_macd_signal))
        .replace("{h4_macd_histogram}", &format!("{:.4}", ctx.ind.h4_macd_histogram))
        .replace("{h4_adx}", &format!("{:.2}", ctx.ind.h4_adx))
        .replace("{h4_bb_width_pct}", &format!("{:.2}", ctx.ind.h4_bb_width_pct * 100.0))
        // ── 1h 主周期 ──
        .replace("{h1_current_price}", &format!("{:.2}", ctx.ind.current_price))
        .replace("{h1_ema20}", &format!("{:.2}", ctx.ind.ema20))
        .replace("{h1_ema50}", &format!("{:.2}", ctx.ind.ema50))
        .replace("{h1_ema_cross}", h1_ema_cross)
        .replace("{h1_ema_cross_bars_ago}", &h1_ema_cross_bars)
        .replace("{h1_ema_gap_pct}", &format!("{:.2}", ctx.ind.h1_ema_gap_pct))
        .replace("{h1_ema_gap_trend}", &ctx.ind.h1_ema_gap_trend)
        .replace("{h1_rsi}", &format!("{:.2}", ctx.ind.rsi))
        .replace("{h1_macd}", &format!("{:.4}", ctx.ind.macd))
        .replace("{h1_macd_signal}", &format!("{:.4}", ctx.ind.macd_signal))
        .replace("{h1_macd_histogram}", &format!("{:.4}", ctx.ind.macd_histogram))
        .replace("{h1_adx}", &format!("{:.2}", ctx.ind.adx))
        .replace("{h1_atr}", &format!("{:.4}", ctx.ind.atr))
        .replace("{h1_bb_upper}", &format!("{:.2}", ctx.ind.bb_upper))
        .replace("{h1_bb_middle}", &format!("{:.2}", ctx.ind.bb_middle))
        .replace("{h1_bb_lower}", &format!("{:.2}", ctx.ind.bb_lower))
        .replace("{h1_bb_width_pct}", &format!("{:.2}", ctx.ind.bb_width * 100.0))
        .replace("{h1_change}", &format!("{:+.2}", ctx.ind.change_1h))
        .replace("{h1_volume}", &format!("{:.2}", ctx.ind.h1_volume))
        .replace("{h1_volume_sma20}", &format!("{:.2}", ctx.ind.h1_volume_sma20))
        .replace("{h1_high_50}", &format!("{:.2}", ctx.ind.h1_high_50))
        .replace("{h1_low_50}", &format!("{:.2}", ctx.ind.h1_low_50))
        .replace("{h1_high_20}", &format!("{:.2}", ctx.ind.h1_high_20))
        .replace("{h1_low_20}", &format!("{:.2}", ctx.ind.h1_low_20))
        .replace("{h1_atr_sma20}", &format!("{:.4}", ctx.ind.h1_atr_sma20))
        .replace("{h1_candle_body}", &format!("{:.4}", ctx.ind.h1_candle_body))
        .replace("{h1_bars_outside_band}", &format_bars_outside(ctx.ind.h1_bars_outside_band))
        .replace("{h1_bandwidth_5bars_ago}", &format!("{:.2}", ctx.ind.h1_bandwidth_5bars_ago * 100.0))
        .replace("{nearest_round_up}", &format!("{:.2}", ctx.ind.nearest_round_up))
        .replace("{nearest_round_down}", &format!("{:.2}", ctx.ind.nearest_round_down))
        // ── 15m 入场周期 ──
        .replace("{m15_current_price}", &format!("{:.2}", ctx.ind.m15_current_price))
        .replace("{m15_ema20}", &format!("{:.2}", ctx.ind.m15_ema20))
        .replace("{m15_ema50}", &format!("{:.2}", ctx.ind.m15_ema50))
        .replace("{m15_ema_cross}", m15_ema_cross)
        .replace("{m15_ema_cross_bars_ago}", &m15_ema_cross_bars)
        .replace("{m15_rsi}", &format!("{:.2}", ctx.ind.m15_rsi))
        .replace("{m15_macd}", &format!("{:.4}", ctx.ind.m15_macd))
        .replace("{m15_macd_signal}", &format!("{:.4}", ctx.ind.m15_macd_signal))
        .replace("{m15_macd_histogram}", &format!("{:.4}", ctx.ind.m15_macd_histogram))
        .replace("{m15_atr}", &format!("{:.4}", ctx.ind.m15_atr))
        .replace("{m15_adx}", &format!("{:.2}", ctx.ind.m15_adx))
        .replace("{m15_bb_width_pct}", &format!("{:.2}", ctx.ind.m15_bb_width_pct * 100.0))
        .replace("{m15_atr_sma20}", &format!("{:.4}", ctx.ind.m15_atr_sma20))
        .replace("{m15_bars_outside_band}", &format_bars_outside(ctx.ind.m15_bars_outside_band))
        .replace("{m15_volume}", &format!("{:.2}", ctx.ind.m15_volume))
        .replace("{m15_volume_sma20}", &format!("{:.2}", ctx.ind.m15_volume_sma20))
        .replace("{m15_high_50}", &format!("{:.2}", ctx.ind.m15_high_50))
        .replace("{m15_low_50}", &format!("{:.2}", ctx.ind.m15_low_50))
}

/// 格式化连续出轨方向及根数。
///
/// 正数表示向上出轨，负数表示向下出轨，0 表示无。
pub fn format_bars_outside(count: i32) -> String {
    if count > 0 {
        format!("向上{}根", count)
    } else if count < 0 {
        format!("向下{}根", count.abs())
    } else {
        "无".to_string()
    }
}
