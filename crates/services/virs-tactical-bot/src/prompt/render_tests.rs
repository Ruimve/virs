use virs_indicator::{IndicatorSet, IndicatorValue, IndicatorSpec};
use virs_type::Timeframe;
use crate::prompt::render::{format_bars_outside, render, RenderContext};

/// 测试辅助：构造含特定指标值的 IndicatorSet。
fn make_indicators() -> IndicatorSet {
    IndicatorSet::with_value(IndicatorSpec::CurrentPrice { tf: Timeframe::H1 }, IndicatorValue::Num(50000.0))
        .insert(IndicatorSpec::Ema { tf: Timeframe::H1, period: 20 }, IndicatorValue::Num(49500.0))
        .insert(IndicatorSpec::Ema { tf: Timeframe::H1, period: 50 }, IndicatorValue::Num(49000.0))
        .insert(IndicatorSpec::EmaCrossState { tf: Timeframe::H1, fast: 20, slow: 50 }, IndicatorValue::Str("金叉(多头)".to_string()))
        .insert(IndicatorSpec::EmaCrossBarsAgo { tf: Timeframe::H1, fast: 20, slow: 50 }, IndicatorValue::Int(5))
        .insert(IndicatorSpec::CurrentPrice { tf: Timeframe::M15 }, IndicatorValue::Num(50000.0))
        .insert(IndicatorSpec::Ema { tf: Timeframe::M15, period: 20 }, IndicatorValue::Num(50100.0))
        .insert(IndicatorSpec::Ema { tf: Timeframe::M15, period: 50 }, IndicatorValue::Num(49900.0))
        .insert(IndicatorSpec::EmaCrossState { tf: Timeframe::M15, fast: 20, slow: 50 }, IndicatorValue::Str("金叉(多头)".to_string()))
        .insert(IndicatorSpec::EmaCrossBarsAgo { tf: Timeframe::M15, fast: 20, slow: 50 }, IndicatorValue::Int(3))
        .insert(IndicatorSpec::BarsOutsideBand { tf: Timeframe::H1, period: 20, stddev: 2 }, IndicatorValue::Int(2))
        .insert(IndicatorSpec::BarsOutsideBand { tf: Timeframe::M15, period: 20, stddev: 2 }, IndicatorValue::Int(-1))
        .clone()
}

fn make_ctx() -> RenderContext {
    RenderContext {
        timestamp: "2026-07-19 12:00:00 UTC".to_string(),
        symbol: "BTC/USDT".to_string(),
        exchange: "binance".to_string(),
        total_balance: 10000.0,
        available_balance: 5000.0,
        used_margin: 3000.0,
        margin_usage_rate: 30.0,
        leverage: 10,
        funding_rate: 0.0001,
        funding_next_time: "2026-07-19 16:00:00".to_string(),
        total_trades: 50,
        win_trades: 30,
        loss_trades: 20,
        total_pnl: 500.0,
        consecutive_losses: 2,
        event_flag: false,
        event_description: String::new(),
        trigger_reason: "scheduled".to_string(),
        min_qty: 0.001,
        position_info: "无仓位".to_string(),
        position_duration: "无".to_string(),
        stop_take_profit_info: "未设置".to_string(),
        recent_close_info: "无".to_string(),
        ind: make_indicators(),
    }
}

#[test]
fn r1_replaces_account_placeholders() {
    let ctx = make_ctx();
    let result = render("{total_balance} {available_balance} {used_margin} {margin_usage_rate} {leverage}", &ctx);
    assert_eq!(result, "10000.00 5000.00 3000.00 30.0 10");
}

#[test]
fn r2_replaces_symbol_placeholders() {
    let ctx = make_ctx();
    let result = render("{symbol} {exchange} {min_qty}", &ctx);
    assert_eq!(result, "BTC/USDT binance 0.001000");
}

#[test]
fn r3_replaces_funding_rate_as_percentage() {
    let ctx = make_ctx();
    let result = render("{funding_rate} {funding_next_time}", &ctx);
    assert_eq!(result, "0.0100% 2026-07-19 16:00:00");
}

#[test]
fn r4_replaces_h1_indicators() {
    let ctx = make_ctx();
    let result = render("{h1_current_price} {h1_ema20} {h1_ema50} {h1_ema_cross} {h1_ema_cross_bars_ago}", &ctx);
    assert_eq!(result, "50000.00 49500.00 49000.00 金叉(多头) 5");
}

#[test]
fn r5_replaces_m15_indicators() {
    let ctx = make_ctx();
    let result = render("{m15_current_price} {m15_ema_cross} {m15_ema_cross_bars_ago}", &ctx);
    assert_eq!(result, "50000.00 金叉(多头) 3");
}

#[test]
fn r6_replaces_bars_outside_band() {
    let ctx = make_ctx();
    let result = render("{h1_bars_outside_band} {m15_bars_outside_band}", &ctx);
    assert_eq!(result, "向上2根 向下1根");
}

#[test]
fn r8_replaces_statistics() {
    let ctx = make_ctx();
    let result = render("{total_trades} {win_trades} {loss_trades} {total_pnl} {consecutive_losses}", &ctx);
    assert_eq!(result, "50 30 20 500.00 2");
}

#[test]
fn r9_no_op_for_absent_placeholders() {
    let ctx = make_ctx();
    let result = render("hello world", &ctx);
    assert_eq!(result, "hello world");
}

#[test]
fn r10_ema_cross_bars_none_when_negative() {
    let ind = IndicatorSet::with_value(IndicatorSpec::EmaCrossBarsAgo { tf: Timeframe::H1, fast: 20, slow: 50 }, IndicatorValue::Int(-1))
        .insert(IndicatorSpec::EmaCrossBarsAgo { tf: Timeframe::M15, fast: 20, slow: 50 }, IndicatorValue::Int(-1))
        .clone();
    let ctx = RenderContext {
        timestamp: String::new(),
        symbol: String::new(),
        exchange: String::new(),
        total_balance: 0.0,
        available_balance: 0.0,
        used_margin: 0.0,
        margin_usage_rate: 0.0,
        leverage: 0,
        funding_rate: 0.0,
        funding_next_time: String::new(),
        total_trades: 0,
        win_trades: 0,
        loss_trades: 0,
        total_pnl: 0.0,
        consecutive_losses: 0,
        event_flag: false,
        event_description: String::new(),
        trigger_reason: String::new(),
        min_qty: 0.0,
        position_info: String::new(),
        position_duration: String::new(),
        stop_take_profit_info: String::new(),
        recent_close_info: String::new(),
        ind,
    };
    let result = render("{h1_ema_cross_bars_ago} {m15_ema_cross_bars_ago}", &ctx);
    assert_eq!(result, "无近期交叉 无近期交叉");
}

#[test]
fn r11_format_bars_positive() {
    assert_eq!(format_bars_outside(3), "向上3根");
}

#[test]
fn r12_format_bars_negative() {
    assert_eq!(format_bars_outside(-2), "向下2根");
}

#[test]
fn r13_format_bars_zero() {
    assert_eq!(format_bars_outside(0), "无");
}
