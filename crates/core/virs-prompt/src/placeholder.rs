

use std::collections::HashSet;

use virs_type::IndicatorSpec;
use virs_type::Timeframe;


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    General,
    Position,
    H1,
    M15,
    H4,
    Event,
}

impl Category {
    pub fn label(&self) -> &'static str {
        match self {
            Category::General => "通用",
            Category::Position => "仓位",
            Category::H1 => "H1 指标",
            Category::M15 => "M15 指标",
            Category::H4 => "H4 指标",
            Category::Event => "事件",
        }
    }
}


/* 占位符格式化枚举：定义数值/字符串的输出格式，如 Price2 保留两位小数、Percent4 保留四位百分比 */
#[derive(Debug, Clone, Copy)]
pub enum Format {

    Price2,

    Price4,

    Price6,

    Signed2,

    Decimal1,

    Percent2,

    Percent4,

    Int,

    Str,

    Bool,

    BarsOutside,

    CrossBarsAgo,
}

impl Format {
    /* 数值格式化：Percent2 乘以 100 但不附加 %（调用方自行处理），Percent4 乘以 100 并附加 % */
    pub fn apply_num(&self, v: f64) -> String {
        match self {
            Format::Price2 => format!("{:.2}", v),
            Format::Price4 => format!("{:.4}", v),
            Format::Price6 => format!("{:.6}", v),
            Format::Signed2 => format!("{:+.2}", v),
            Format::Decimal1 => format!("{:.1}", v),
            Format::Percent2 => format!("{:.2}", v * 100.0),
            Format::Percent4 => format!("{:.4}%", v * 100.0),
            _ => format!("{:.2}", v),
        }
    }

    /* 整数格式化：BarsOutside 正数为"向上N根"，负数为"向下N根"；CrossBarsAgo -1 为"无近期交叉" */
    pub fn apply_int(&self, v: i32) -> String {
        match self {
            Format::Int => v.to_string(),
            Format::BarsOutside => {
                if v > 0 {
                    format!("向上{}根", v)
                } else if v < 0 {
                    format!("向下{}根", v.abs())
                } else {
                    "无".to_string()
                }
            }
            Format::CrossBarsAgo => {
                if v >= 0 {
                    v.to_string()
                } else {
                    "无近期交叉".to_string()
                }
            }
            _ => v.to_string(),
        }
    }

    pub fn apply_str(&self, v: &str) -> String {
        v.to_string()
    }

    pub fn apply_bool(&self, v: bool) -> String {
        if v { "true".to_string() } else { "false".to_string() }
    }
}


#[derive(Debug, Clone, Copy)]
pub enum ContextField {
    Timestamp,
    Symbol,
    Exchange,
    MinQty,
    TotalBalance,
    AvailableBalance,
    UsedMargin,
    MarginUsageRate,
    Leverage,
    PositionInfo,
    PositionDuration,
    StopTakeProfitInfo,
    RecentCloseInfo,
    ConsecutiveLosses,
    FundingRate,
    FundingNextTime,
    TotalTrades,
    WinTrades,
    LossTrades,
    TotalPnl,
    EventFlag,
    EventDescription,
    TriggerReason,
}


/* 占位符来源：Context（运行时上下文字段）或 Indicator（技术指标计算结果） */
#[derive(Debug, Clone)]
pub enum PlaceholderSource {
    Context(ContextField, Format),
    Indicator(IndicatorSpec, Format),
}


#[derive(Debug, Clone)]
pub struct PlaceholderDef {
    pub name: &'static str,
    pub source: PlaceholderSource,
    pub category: Category,
}


/* 占位符注册表：定义所有可用占位符的名称、数据来源和格式，分为通用、仓位、H1/M15/H4 指标、事件六大类 */
pub const REGISTRY: &[PlaceholderDef] = &[

    PlaceholderDef { name: "timestamp",         source: PlaceholderSource::Context(ContextField::Timestamp, Format::Str),       category: Category::General },
    PlaceholderDef { name: "symbol",            source: PlaceholderSource::Context(ContextField::Symbol, Format::Str),         category: Category::General },
    PlaceholderDef { name: "exchange",          source: PlaceholderSource::Context(ContextField::Exchange, Format::Str),       category: Category::General },
    PlaceholderDef { name: "leverage",          source: PlaceholderSource::Context(ContextField::Leverage, Format::Int),        category: Category::General },
    PlaceholderDef { name: "total_balance",     source: PlaceholderSource::Context(ContextField::TotalBalance, Format::Price2), category: Category::General },
    PlaceholderDef { name: "available_balance", source: PlaceholderSource::Context(ContextField::AvailableBalance, Format::Price2), category: Category::General },
    PlaceholderDef { name: "used_margin",       source: PlaceholderSource::Context(ContextField::UsedMargin, Format::Price2),   category: Category::General },
    PlaceholderDef { name: "margin_usage_rate", source: PlaceholderSource::Context(ContextField::MarginUsageRate, Format::Decimal1), category: Category::General },
    PlaceholderDef { name: "min_qty",           source: PlaceholderSource::Context(ContextField::MinQty, Format::Price6),       category: Category::General },
    PlaceholderDef { name: "funding_rate",      source: PlaceholderSource::Context(ContextField::FundingRate, Format::Percent4), category: Category::General },
    PlaceholderDef { name: "funding_next_time", source: PlaceholderSource::Context(ContextField::FundingNextTime, Format::Str), category: Category::General },


    PlaceholderDef { name: "position_info",        source: PlaceholderSource::Context(ContextField::PositionInfo, Format::Str),        category: Category::Position },
    PlaceholderDef { name: "position_duration",    source: PlaceholderSource::Context(ContextField::PositionDuration, Format::Str),    category: Category::Position },
    PlaceholderDef { name: "stop_take_profit_info",source: PlaceholderSource::Context(ContextField::StopTakeProfitInfo, Format::Str),  category: Category::Position },
    PlaceholderDef { name: "recent_close_info",    source: PlaceholderSource::Context(ContextField::RecentCloseInfo, Format::Str),     category: Category::Position },
    PlaceholderDef { name: "total_trades",         source: PlaceholderSource::Context(ContextField::TotalTrades, Format::Int),         category: Category::Position },
    PlaceholderDef { name: "win_trades",           source: PlaceholderSource::Context(ContextField::WinTrades, Format::Int),           category: Category::Position },
    PlaceholderDef { name: "loss_trades",          source: PlaceholderSource::Context(ContextField::LossTrades, Format::Int),          category: Category::Position },
    PlaceholderDef { name: "total_pnl",            source: PlaceholderSource::Context(ContextField::TotalPnl, Format::Price2),         category: Category::Position },
    PlaceholderDef { name: "consecutive_losses",   source: PlaceholderSource::Context(ContextField::ConsecutiveLosses, Format::Int),   category: Category::Position },
    PlaceholderDef { name: "trigger_reason",       source: PlaceholderSource::Context(ContextField::TriggerReason, Format::Str),       category: Category::Position },


    PlaceholderDef { name: "h1_current_price",       source: PlaceholderSource::Indicator(IndicatorSpec::CurrentPrice { tf: Timeframe::H1 }, Format::Price2),     category: Category::H1 },
    PlaceholderDef { name: "h1_rsi",                 source: PlaceholderSource::Indicator(IndicatorSpec::Rsi { tf: Timeframe::H1, period: 14 }, Format::Price2),            category: Category::H1 },
    PlaceholderDef { name: "h1_atr",                 source: PlaceholderSource::Indicator(IndicatorSpec::Atr { tf: Timeframe::H1, period: 14 }, Format::Price4),            category: Category::H1 },
    PlaceholderDef { name: "h1_adx",                 source: PlaceholderSource::Indicator(IndicatorSpec::Adx { tf: Timeframe::H1, period: 14 }, Format::Price2),            category: Category::H1 },
    PlaceholderDef { name: "h1_macd",                source: PlaceholderSource::Indicator(IndicatorSpec::Macd { tf: Timeframe::H1, fast: 12, slow: 26, signal: 9 }, Format::Price4),   category: Category::H1 },
    PlaceholderDef { name: "h1_macd_signal",         source: PlaceholderSource::Indicator(IndicatorSpec::MacdSignal { tf: Timeframe::H1, fast: 12, slow: 26, signal: 9 }, Format::Price4), category: Category::H1 },
    PlaceholderDef { name: "h1_macd_histogram",      source: PlaceholderSource::Indicator(IndicatorSpec::MacdHistogram { tf: Timeframe::H1, fast: 12, slow: 26, signal: 9 }, Format::Price4), category: Category::H1 },
    PlaceholderDef { name: "h1_ema20",               source: PlaceholderSource::Indicator(IndicatorSpec::Ema { tf: Timeframe::H1, period: 20 }, Format::Price2),           category: Category::H1 },
    PlaceholderDef { name: "h1_ema50",               source: PlaceholderSource::Indicator(IndicatorSpec::Ema { tf: Timeframe::H1, period: 50 }, Format::Price2),           category: Category::H1 },
    PlaceholderDef { name: "h1_ema_cross",           source: PlaceholderSource::Indicator(IndicatorSpec::EmaCrossState { tf: Timeframe::H1, fast: 20, slow: 50 }, Format::Str), category: Category::H1 },
    PlaceholderDef { name: "h1_ema_cross_bars_ago",  source: PlaceholderSource::Indicator(IndicatorSpec::EmaCrossBarsAgo { tf: Timeframe::H1, fast: 20, slow: 50 }, Format::CrossBarsAgo), category: Category::H1 },
    PlaceholderDef { name: "h1_ema_gap_pct",         source: PlaceholderSource::Indicator(IndicatorSpec::EmaGapPct { tf: Timeframe::H1, fast: 20, slow: 50 }, Format::Price2), category: Category::H1 },
    PlaceholderDef { name: "h1_ema_gap_trend",       source: PlaceholderSource::Indicator(IndicatorSpec::EmaGapTrend { tf: Timeframe::H1, fast: 20, slow: 50 }, Format::Str), category: Category::H1 },
    PlaceholderDef { name: "h1_bb_upper",            source: PlaceholderSource::Indicator(IndicatorSpec::BbandsUpper { tf: Timeframe::H1, period: 20, stddev: 2 }, Format::Price2), category: Category::H1 },
    PlaceholderDef { name: "h1_bb_middle",           source: PlaceholderSource::Indicator(IndicatorSpec::BbandsMiddle { tf: Timeframe::H1, period: 20, stddev: 2 }, Format::Price2), category: Category::H1 },
    PlaceholderDef { name: "h1_bb_lower",            source: PlaceholderSource::Indicator(IndicatorSpec::BbandsLower { tf: Timeframe::H1, period: 20, stddev: 2 }, Format::Price2), category: Category::H1 },
    PlaceholderDef { name: "h1_bb_width_pct",        source: PlaceholderSource::Indicator(IndicatorSpec::BbandsWidth { tf: Timeframe::H1, period: 20, stddev: 2 }, Format::Percent2), category: Category::H1 },
    PlaceholderDef { name: "h1_change",              source: PlaceholderSource::Indicator(IndicatorSpec::ChangePct { tf: Timeframe::H1, period: 1 }, Format::Signed2), category: Category::H1 },
    PlaceholderDef { name: "h1_volume",              source: PlaceholderSource::Indicator(IndicatorSpec::LastCompletedVolume { tf: Timeframe::H1 }, Format::Price2), category: Category::H1 },
    PlaceholderDef { name: "h1_volume_sma20",        source: PlaceholderSource::Indicator(IndicatorSpec::VolumeSma { tf: Timeframe::H1, period: 20 }, Format::Price2), category: Category::H1 },
    PlaceholderDef { name: "h1_candle_body",         source: PlaceholderSource::Indicator(IndicatorSpec::CandleBody { tf: Timeframe::H1 }, Format::Price4), category: Category::H1 },
    PlaceholderDef { name: "h1_bars_outside_band",   source: PlaceholderSource::Indicator(IndicatorSpec::BarsOutsideBand { tf: Timeframe::H1, period: 20, stddev: 2 }, Format::BarsOutside), category: Category::H1 },
    PlaceholderDef { name: "h1_bandwidth_5bars_ago", source: PlaceholderSource::Indicator(IndicatorSpec::BandwidthBarsAgo { tf: Timeframe::H1, period: 20, stddev: 2, bars_ago: 5 }, Format::Percent2), category: Category::H1 },
    PlaceholderDef { name: "h1_high_20",             source: PlaceholderSource::Indicator(IndicatorSpec::Highest { tf: Timeframe::H1, period: 20 }, Format::Price2), category: Category::H1 },
    PlaceholderDef { name: "h1_low_20",              source: PlaceholderSource::Indicator(IndicatorSpec::Lowest { tf: Timeframe::H1, period: 20 }, Format::Price2), category: Category::H1 },
    PlaceholderDef { name: "h1_high_50",             source: PlaceholderSource::Indicator(IndicatorSpec::Highest { tf: Timeframe::H1, period: 50 }, Format::Price2), category: Category::H1 },
    PlaceholderDef { name: "h1_low_50",              source: PlaceholderSource::Indicator(IndicatorSpec::Lowest { tf: Timeframe::H1, period: 50 }, Format::Price2), category: Category::H1 },
    PlaceholderDef { name: "h1_atr_sma20",           source: PlaceholderSource::Indicator(IndicatorSpec::AtrSma { tf: Timeframe::H1, atr_period: 14, sma_period: 20 }, Format::Price4), category: Category::H1 },
    PlaceholderDef { name: "nearest_round_up",       source: PlaceholderSource::Indicator(IndicatorSpec::RoundNumberUp, Format::Price2), category: Category::H1 },
    PlaceholderDef { name: "nearest_round_down",     source: PlaceholderSource::Indicator(IndicatorSpec::RoundNumberDown, Format::Price2), category: Category::H1 },


    PlaceholderDef { name: "m15_current_price",      source: PlaceholderSource::Indicator(IndicatorSpec::CurrentPrice { tf: Timeframe::M15 }, Format::Price2), category: Category::M15 },
    PlaceholderDef { name: "m15_rsi",                source: PlaceholderSource::Indicator(IndicatorSpec::Rsi { tf: Timeframe::M15, period: 14 }, Format::Price2), category: Category::M15 },
    PlaceholderDef { name: "m15_macd",               source: PlaceholderSource::Indicator(IndicatorSpec::Macd { tf: Timeframe::M15, fast: 12, slow: 26, signal: 9 }, Format::Price4), category: Category::M15 },
    PlaceholderDef { name: "m15_macd_signal",        source: PlaceholderSource::Indicator(IndicatorSpec::MacdSignal { tf: Timeframe::M15, fast: 12, slow: 26, signal: 9 }, Format::Price4), category: Category::M15 },
    PlaceholderDef { name: "m15_macd_histogram",     source: PlaceholderSource::Indicator(IndicatorSpec::MacdHistogram { tf: Timeframe::M15, fast: 12, slow: 26, signal: 9 }, Format::Price4), category: Category::M15 },
    PlaceholderDef { name: "m15_atr",                source: PlaceholderSource::Indicator(IndicatorSpec::Atr { tf: Timeframe::M15, period: 14 }, Format::Price4), category: Category::M15 },
    PlaceholderDef { name: "m15_adx",                source: PlaceholderSource::Indicator(IndicatorSpec::Adx { tf: Timeframe::M15, period: 14 }, Format::Price2), category: Category::M15 },
    PlaceholderDef { name: "m15_bb_width_pct",       source: PlaceholderSource::Indicator(IndicatorSpec::BbandsWidth { tf: Timeframe::M15, period: 20, stddev: 2 }, Format::Percent2), category: Category::M15 },
    PlaceholderDef { name: "m15_atr_sma20",          source: PlaceholderSource::Indicator(IndicatorSpec::AtrSma { tf: Timeframe::M15, atr_period: 14, sma_period: 20 }, Format::Price4), category: Category::M15 },
    PlaceholderDef { name: "m15_bars_outside_band",  source: PlaceholderSource::Indicator(IndicatorSpec::BarsOutsideBand { tf: Timeframe::M15, period: 20, stddev: 2 }, Format::BarsOutside), category: Category::M15 },
    PlaceholderDef { name: "m15_ema20",              source: PlaceholderSource::Indicator(IndicatorSpec::Ema { tf: Timeframe::M15, period: 20 }, Format::Price2), category: Category::M15 },
    PlaceholderDef { name: "m15_ema50",              source: PlaceholderSource::Indicator(IndicatorSpec::Ema { tf: Timeframe::M15, period: 50 }, Format::Price2), category: Category::M15 },
    PlaceholderDef { name: "m15_ema_cross",          source: PlaceholderSource::Indicator(IndicatorSpec::EmaCrossState { tf: Timeframe::M15, fast: 20, slow: 50 }, Format::Str), category: Category::M15 },
    PlaceholderDef { name: "m15_ema_cross_bars_ago", source: PlaceholderSource::Indicator(IndicatorSpec::EmaCrossBarsAgo { tf: Timeframe::M15, fast: 20, slow: 50 }, Format::CrossBarsAgo), category: Category::M15 },
    PlaceholderDef { name: "m15_volume",             source: PlaceholderSource::Indicator(IndicatorSpec::LastCompletedVolume { tf: Timeframe::M15 }, Format::Price2), category: Category::M15 },
    PlaceholderDef { name: "m15_volume_sma20",       source: PlaceholderSource::Indicator(IndicatorSpec::VolumeSma { tf: Timeframe::M15, period: 20 }, Format::Price2), category: Category::M15 },
    PlaceholderDef { name: "m15_high_50",            source: PlaceholderSource::Indicator(IndicatorSpec::Highest { tf: Timeframe::M15, period: 50 }, Format::Price2), category: Category::M15 },
    PlaceholderDef { name: "m15_low_50",             source: PlaceholderSource::Indicator(IndicatorSpec::Lowest { tf: Timeframe::M15, period: 50 }, Format::Price2), category: Category::M15 },
    PlaceholderDef { name: "m15_bb_upper",           source: PlaceholderSource::Indicator(IndicatorSpec::BbandsUpper { tf: Timeframe::M15, period: 20, stddev: 2 }, Format::Price2), category: Category::M15 },
    PlaceholderDef { name: "m15_bb_middle",          source: PlaceholderSource::Indicator(IndicatorSpec::BbandsMiddle { tf: Timeframe::M15, period: 20, stddev: 2 }, Format::Price2), category: Category::M15 },
    PlaceholderDef { name: "m15_bb_lower",           source: PlaceholderSource::Indicator(IndicatorSpec::BbandsLower { tf: Timeframe::M15, period: 20, stddev: 2 }, Format::Price2), category: Category::M15 },
    PlaceholderDef { name: "m15_change",             source: PlaceholderSource::Indicator(IndicatorSpec::ChangePct { tf: Timeframe::M15, period: 1 }, Format::Signed2), category: Category::M15 },
    PlaceholderDef { name: "m15_candle_body",        source: PlaceholderSource::Indicator(IndicatorSpec::CandleBody { tf: Timeframe::M15 }, Format::Price4), category: Category::M15 },
    PlaceholderDef { name: "m15_high_20",            source: PlaceholderSource::Indicator(IndicatorSpec::Highest { tf: Timeframe::M15, period: 20 }, Format::Price2), category: Category::M15 },
    PlaceholderDef { name: "m15_low_20",             source: PlaceholderSource::Indicator(IndicatorSpec::Lowest { tf: Timeframe::M15, period: 20 }, Format::Price2), category: Category::M15 },
    PlaceholderDef { name: "m15_bandwidth_5bars_ago",source: PlaceholderSource::Indicator(IndicatorSpec::BandwidthBarsAgo { tf: Timeframe::M15, period: 20, stddev: 2, bars_ago: 5 }, Format::Percent2), category: Category::M15 },
    PlaceholderDef { name: "m15_ema_gap_pct",        source: PlaceholderSource::Indicator(IndicatorSpec::EmaGapPct { tf: Timeframe::M15, fast: 20, slow: 50 }, Format::Price2), category: Category::M15 },


    PlaceholderDef { name: "h4_ema20",           source: PlaceholderSource::Indicator(IndicatorSpec::Ema { tf: Timeframe::H4, period: 20 }, Format::Price2), category: Category::H4 },
    PlaceholderDef { name: "h4_ema50",           source: PlaceholderSource::Indicator(IndicatorSpec::Ema { tf: Timeframe::H4, period: 50 }, Format::Price2), category: Category::H4 },
    PlaceholderDef { name: "h4_adx",             source: PlaceholderSource::Indicator(IndicatorSpec::Adx { tf: Timeframe::H4, period: 14 }, Format::Price2), category: Category::H4 },
    PlaceholderDef { name: "h4_rsi",             source: PlaceholderSource::Indicator(IndicatorSpec::Rsi { tf: Timeframe::H4, period: 14 }, Format::Price2), category: Category::H4 },
    PlaceholderDef { name: "h4_macd",            source: PlaceholderSource::Indicator(IndicatorSpec::Macd { tf: Timeframe::H4, fast: 12, slow: 26, signal: 9 }, Format::Price4), category: Category::H4 },
    PlaceholderDef { name: "h4_macd_signal",     source: PlaceholderSource::Indicator(IndicatorSpec::MacdSignal { tf: Timeframe::H4, fast: 12, slow: 26, signal: 9 }, Format::Price4), category: Category::H4 },
    PlaceholderDef { name: "h4_macd_histogram",  source: PlaceholderSource::Indicator(IndicatorSpec::MacdHistogram { tf: Timeframe::H4, fast: 12, slow: 26, signal: 9 }, Format::Price4), category: Category::H4 },
    PlaceholderDef { name: "h4_bb_width_pct",    source: PlaceholderSource::Indicator(IndicatorSpec::BbandsWidth { tf: Timeframe::H4, period: 20, stddev: 2 }, Format::Percent2), category: Category::H4 },


    PlaceholderDef { name: "event_flag",        source: PlaceholderSource::Context(ContextField::EventFlag, Format::Bool),       category: Category::Event },
    PlaceholderDef { name: "event_description", source: PlaceholderSource::Context(ContextField::EventDescription, Format::Str), category: Category::Event },
];


pub fn all() -> &'static [PlaceholderDef] {
    REGISTRY
}


pub fn names() -> HashSet<&'static str> {
    REGISTRY.iter().map(|d| d.name).collect()
}


/* 生成占位符清单文本：按分类列出所有可用占位符名称，供 AI 生成策略模板时参考 */
pub fn to_prompt_text() -> String {
    use Category::*;
    let categories = [General, Position, H1, M15, H4, Event];
    let mut lines = Vec::new();
    for cat in categories {
        let names: Vec<&str> = REGISTRY
            .iter()
            .filter(|d| d.category == cat)
            .map(|d| d.name)
            .collect();
        lines.push(format!("- {}: {}", cat.label(), names.join(", ")));
    }
    lines.join("\n")
}
