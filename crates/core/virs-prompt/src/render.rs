

use virs_indicator::{IndicatorSet, IndicatorValue};

use crate::placeholder::{ContextField, Format, PlaceholderSource};


/* 提示词渲染上下文：包含运行时交易状态（余额、持仓、事件等）和技术指标集，用于占位符替换 */
#[derive(Debug, Clone)]
pub struct RenderContext {

    pub timestamp: String,


    pub symbol: String,
    pub exchange: String,
    pub min_qty: f64,


    pub total_balance: f64,
    pub available_balance: f64,
    pub used_margin: f64,
    pub margin_usage_rate: f64,
    pub leverage: i32,


    pub position_info: String,
    pub position_duration: String,
    pub stop_take_profit_info: String,
    pub recent_close_info: String,
    pub consecutive_losses: i32,


    pub funding_rate: f64,
    pub funding_next_time: String,


    pub total_trades: i32,
    pub win_trades: i32,
    pub loss_trades: i32,
    pub total_pnl: f64,


    pub event_flag: bool,
    pub event_description: String,


    pub trigger_reason: String,


    pub ind: IndicatorSet,
}


fn context_value(ctx: &RenderContext, field: ContextField, format: Format) -> String {
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


fn indicator_value(set: &IndicatorSet, spec: &virs_indicator::IndicatorSpec, format: Format) -> Option<String> {
    match set.get(spec)? {
        IndicatorValue::Num(v) => Some(format.apply_num(*v)),
        IndicatorValue::Int(v) => Some(format.apply_int(*v)),
        IndicatorValue::Str(v) => Some(format.apply_str(v)),
    }
}


/* 模板渲染：遍历所有已注册占位符，将模板中的 {placeholder} 替换为上下文或指标计算结果 */
pub fn render(template: &str, ctx: &RenderContext) -> String {
    let mut result = template.to_string();
    for def in crate::placeholder::all() {
        let replacement = match &def.source {
            PlaceholderSource::Context(field, format) => {
                context_value(ctx, *field, *format)
            }
            PlaceholderSource::Indicator(spec, format) => {
                /* 指标值缺失时跳过替换，保留原始占位符文本 */
                match indicator_value(&ctx.ind, spec, *format) {
                    Some(s) => s,
                    None => continue,
                }
            }
        };
        /* 将 {占位符名} 替换为格式化后的值 */
        result = result.replace(&format!("{{{}}}", def.name), &replacement);
    }
    result
}


pub fn format_bars_outside(count: i32) -> String {
    Format::BarsOutside.apply_int(count)
}
