/**
 * 测试 strategy::render_prompt 模板渲染
 * - 单个 {placeholder} 被正确替换
 * - 数值格式化正确（2位小数、1位百分比、4位百分比等）
 * - EMA 交叉状态文字正确（金叉/死叉）
 * - EMA 交叉 bars_ago 数字/无近期交叉
 * - m15 EMA 交叉状态
 * - 完整模板无残留占位符
 */
use crate::bot::auto_trade::strategy::{render_prompt, PromptContext};
use crate::bot::common::indicators::MarketIndicators;

fn default_ctx() -> PromptContext {
    PromptContext {
        timestamp: "2025-01-01 00:00:00".to_string(),
        symbol: "BTC/USDT".to_string(),
        exchange: "binance".to_string(),
        market_type: "perpetual".to_string(),
        total_balance: 100.0,
        available_balance: 80.0,
        used_margin: 20.0,
        margin_usage_rate: 0.2,
        leverage: 3,
        position_info: "无仓位".to_string(),
        position_duration: "无持仓".to_string(),
        stop_take_profit_info: "未设置".to_string(),
        funding_rate: 0.0001,
        funding_next_time: "2025-01-01 08:00:00".to_string(),
        total_trades: 10,
        win_trades: 6,
        loss_trades: 4,
        total_pnl: 5.5,
        consecutive_losses: 0,
        trigger_reason: "scheduled".to_string(),
        ind: MarketIndicators::default(),
        min_qty: 0.001,
    }
}

#[test]
fn replaces_timestamp() {
    let ctx = default_ctx();
    assert_eq!(render_prompt("{timestamp}", &ctx), "2025-01-01 00:00:00");
}

#[test]
fn replaces_symbol() {
    let ctx = default_ctx();
    assert_eq!(render_prompt("{symbol}", &ctx), "BTC/USDT");
}

#[test]
fn replaces_exchange() {
    let ctx = default_ctx();
    assert_eq!(render_prompt("{exchange}", &ctx), "binance");
}

#[test]
fn replaces_market_type() {
    let ctx = default_ctx();
    assert_eq!(render_prompt("{market_type}", &ctx), "perpetual");
}

#[test]
fn replaces_total_balance_two_decimals() {
    let ctx = default_ctx();
    assert_eq!(render_prompt("{total_balance}", &ctx), "100.00");
}

#[test]
fn replaces_available_balance_two_decimals() {
    let ctx = default_ctx();
    assert_eq!(render_prompt("{available_balance}", &ctx), "80.00");
}

#[test]
fn replaces_used_margin_two_decimals() {
    let ctx = default_ctx();
    assert_eq!(render_prompt("{used_margin}", &ctx), "20.00");
}

#[test]
fn replaces_margin_usage_rate_one_decimal_pct() {
    let ctx = default_ctx();
    assert_eq!(render_prompt("{margin_usage_rate}", &ctx), "20.0");
}

#[test]
fn replaces_leverage() {
    let ctx = default_ctx();
    assert_eq!(render_prompt("{leverage}", &ctx), "3");
}

#[test]
fn replaces_position_info() {
    let ctx = default_ctx();
    assert_eq!(render_prompt("{position_info}", &ctx), "无仓位");
}

#[test]
fn replaces_stop_take_profit_info() {
    let ctx = default_ctx();
    assert_eq!(render_prompt("{stop_take_profit_info}", &ctx), "未设置");
}

#[test]
fn replaces_funding_rate_as_pct() {
    let ctx = default_ctx();
    let result = render_prompt("{funding_rate}", &ctx);
    assert_eq!(result, "0.0100%");
}

#[test]
fn replaces_trigger_reason() {
    let ctx = default_ctx();
    assert_eq!(render_prompt("{trigger_reason}", &ctx), "scheduled");
}

#[test]
fn replaces_total_trades() {
    let ctx = default_ctx();
    assert_eq!(render_prompt("{total_trades}", &ctx), "10");
}

#[test]
fn replaces_win_trades() {
    let ctx = default_ctx();
    assert_eq!(render_prompt("{win_trades}", &ctx), "6");
}

#[test]
fn replaces_loss_trades() {
    let ctx = default_ctx();
    assert_eq!(render_prompt("{loss_trades}", &ctx), "4");
}

#[test]
fn replaces_total_pnl_two_decimals() {
    let ctx = default_ctx();
    assert_eq!(render_prompt("{total_pnl}", &ctx), "5.50");
}

#[test]
fn replaces_consecutive_losses() {
    let ctx = default_ctx();
    assert_eq!(render_prompt("{consecutive_losses}", &ctx), "0");
}

#[test]
fn h1_ema_cross_golden_when_ema20_above_ema50() {
    let mut ctx = default_ctx();
    ctx.ind.ema20 = 105.0;
    ctx.ind.ema50 = 100.0;
    let result = render_prompt("{h1_ema_cross}", &ctx);
    assert!(result.contains("金叉"), "expected 金叉, got {}", result);
}

#[test]
fn h1_ema_cross_death_when_ema20_below_ema50() {
    let mut ctx = default_ctx();
    ctx.ind.ema20 = 95.0;
    ctx.ind.ema50 = 100.0;
    let result = render_prompt("{h1_ema_cross}", &ctx);
    assert!(result.contains("死叉"), "expected 死叉, got {}", result);
}

#[test]
fn h1_ema_cross_bars_ago_shows_number() {
    let mut ctx = default_ctx();
    ctx.ind.h1_ema_cross_bars_ago = 3;
    assert_eq!(render_prompt("{h1_ema_cross_bars_ago}", &ctx), "3");
}

#[test]
fn h1_ema_cross_bars_ago_negative_shows_no_cross() {
    let mut ctx = default_ctx();
    ctx.ind.h1_ema_cross_bars_ago = -1;
    assert_eq!(render_prompt("{h1_ema_cross_bars_ago}", &ctx), "无近期交叉");
}

#[test]
fn m15_ema_cross_golden_when_ema20_above_ema50() {
    let mut ctx = default_ctx();
    ctx.ind.m15_ema20 = 105.0;
    ctx.ind.m15_ema50 = 100.0;
    let result = render_prompt("{m15_ema_cross}", &ctx);
    assert!(result.contains("金叉"), "expected 金叉, got {}", result);
}

#[test]
fn m15_ema_cross_death_when_ema20_below_ema50() {
    let mut ctx = default_ctx();
    ctx.ind.m15_ema20 = 95.0;
    ctx.ind.m15_ema50 = 100.0;
    let result = render_prompt("{m15_ema_cross}", &ctx);
    assert!(result.contains("死叉"), "expected 死叉, got {}", result);
}

#[test]
fn m15_ema_cross_bars_ago_shows_number() {
    let mut ctx = default_ctx();
    ctx.ind.m15_ema_cross_bars_ago = 2;
    assert_eq!(render_prompt("{m15_ema_cross_bars_ago}", &ctx), "2");
}

#[test]
fn m15_ema_cross_bars_ago_negative_shows_no_cross() {
    let mut ctx = default_ctx();
    ctx.ind.m15_ema_cross_bars_ago = -1;
    assert_eq!(render_prompt("{m15_ema_cross_bars_ago}", &ctx), "无近期交叉");
}

#[test]
fn full_template_no_unreplaced_placeholders() {
    let ctx = default_ctx();
    let template = crate::bot::auto_trade::types::DEFAULT_USER_PROMPT_TEMPLATE;
    let result = render_prompt(template, &ctx);
    assert!(!result.contains("{"), "unreplaced placeholders found: {}", result);
    assert!(!result.contains("}"), "unreplaced placeholders found: {}", result);
}
