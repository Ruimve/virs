//! 统一 prompt 渲染器。
//!
//! [`RenderContext`] 封装渲染 prompt 所需的全部输入数据（账户、持仓、统计、指标等），
//! [`render`] 将模板中的 `{placeholder}` 占位符替换为 context 的实际值。
//!
//! 渲染逻辑由 `crate::placeholder::registry` 驱动：遍历注册表，
//! 按 `PlaceholderSource` 从 Context 或 IndicatorSet 取值，按 `Format` 格式化。
//! 不再有硬编码的 75 行 `.replace()` 链。

use virs_indicator::{IndicatorSet, IndicatorValue};

use crate::placeholder::{ContextField, Format, PlaceholderSource};

/// 渲染 prompt 所需的全部上下文数据。
///
/// 所有字段对任何 bot 都是通用的；不适用于当前 bot 的字段填默认值即可。
#[derive(Debug, Clone)]
pub struct RenderContext {
    // ── 时间 ──
    pub timestamp: String,

    // ── 交易对信息 ──
    pub symbol: String,
    pub exchange: String,
    pub min_qty: f64,

    // ── 账户资产 ──
    pub total_balance: f64,
    pub available_balance: f64,
    pub used_margin: f64,
    pub margin_usage_rate: f64,
    pub leverage: i32,

    // ── 持仓信息 ──
    pub position_info: String,
    pub position_duration: String,
    pub stop_take_profit_info: String,
    pub recent_close_info: String,
    pub consecutive_losses: i32,

    // ── 资金费率 ──
    pub funding_rate: f64,
    pub funding_next_time: String,

    // ── 交易统计 ──
    pub total_trades: i32,
    pub win_trades: i32,
    pub loss_trades: i32,
    pub total_pnl: f64,

    // ── 事件标记 ──
    pub event_flag: bool,
    pub event_description: String,

    // ── 触发原因 ──
    pub trigger_reason: String,

    // ── 技术指标（多周期） ──
    pub ind: IndicatorSet,
}

/// 从 RenderContext 提取 Context 字段值并格式化。
fn context_value<'a>(ctx: &'a RenderContext, field: ContextField, format: Format) -> String {
    use ContextField::*;
    match field {
        Timestamp         => format.apply_str(&ctx.timestamp),
        Symbol            => format.apply_str(&ctx.symbol),
        Exchange          => format.apply_str(&ctx.exchange),
        MinQty            => format.apply_num(ctx.min_qty),
        TotalBalance      => format.apply_num(ctx.total_balance),
        AvailableBalance  => format.apply_num(ctx.available_balance),
        UsedMargin        => format.apply_num(ctx.used_margin),
        MarginUsageRate   => format.apply_num(ctx.margin_usage_rate),
        Leverage          => format.apply_int(ctx.leverage),
        PositionInfo      => format.apply_str(&ctx.position_info),
        PositionDuration  => format.apply_str(&ctx.position_duration),
        StopTakeProfitInfo=> format.apply_str(&ctx.stop_take_profit_info),
        RecentCloseInfo   => format.apply_str(&ctx.recent_close_info),
        ConsecutiveLosses => format.apply_int(ctx.consecutive_losses),
        FundingRate       => format.apply_num(ctx.funding_rate),
        FundingNextTime   => format.apply_str(&ctx.funding_next_time),
        TotalTrades       => format.apply_int(ctx.total_trades),
        WinTrades         => format.apply_int(ctx.win_trades),
        LossTrades        => format.apply_int(ctx.loss_trades),
        TotalPnl          => format.apply_num(ctx.total_pnl),
        EventFlag         => format.apply_bool(ctx.event_flag),
        EventDescription  => format.apply_str(&ctx.event_description),
        TriggerReason     => format.apply_str(&ctx.trigger_reason),
    }
}

/// 从 IndicatorSet 提取指标值并格式化。
fn indicator_value(set: &IndicatorSet, spec: &virs_indicator::IndicatorSpec, format: Format) -> Option<String> {
    match set.get(spec)? {
        IndicatorValue::Num(v) => Some(format.apply_num(*v)),
        IndicatorValue::Int(v) => Some(format.apply_int(*v)),
        IndicatorValue::Str(v) => Some(format.apply_str(v)),
    }
}

/// 将模板中的 `{placeholder}` 占位符替换为 context 的实际值。
///
/// 遍历 `crate::placeholder::registry` 注册表，按 `PlaceholderSource` 取值并格式化。
/// 模板中未出现的占位符 replace 是 no-op，无副作用。
/// 指标值缺失时跳过替换（保留原 `{placeholder}` 文本）。
pub fn render(template: &str, ctx: &RenderContext) -> String {
    let mut result = template.to_string();
    for def in crate::placeholder::all() {
        let replacement = match &def.source {
            PlaceholderSource::Context(field, format) => {
                context_value(ctx, *field, *format)
            }
            PlaceholderSource::Indicator(spec, format) => {
                match indicator_value(&ctx.ind, spec, *format) {
                    Some(s) => s,
                    None => continue, // 指标缺失，跳过替换
                }
            }
        };
        result = result.replace(&format!("{{{}}}", def.name), &replacement);
    }
    result
}

/// 格式化连续出轨方向及根数。
///
/// 正数表示向上出轨，负数表示向下出轨，0 表示无。
pub fn format_bars_outside(count: i32) -> String {
    Format::BarsOutside.apply_int(count)
}
