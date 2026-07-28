use chrono::Utc;
use uuid::Uuid;

use virs_types::StrategyStatus;

use crate::GridBot;

fn make_grid_bot(
    upper: f64,
    lower: f64,
    grid_count: i32,
    status: StrategyStatus,
    total_pnl: f64,
    initial_capital: f64,
) -> GridBot {
    GridBot {
        id: Uuid::nil(),
        user_id: Uuid::nil(),
        name: "test_bot".into(),
        symbol: "BTC/USDT".into(),
        exchange: "binance".into(),
        paper_mode: false,
        status,
        upper_price: upper,
        lower_price: lower,
        grid_count,
        grid_profit_pct: 0.5,
        quantity_per_grid: 0.01,
        leverage: 10,
        initial_capital,
        market_regime: None,
        ai_analysis: None,
        grid_levels_json: None,
        system_prompt: None,
        user_prompt: None,
        dynamic_adjust: false,
        adjust_interval_secs: 3600,
        last_adjusted_at: None,
        strategy_file: None,
        total_pnl,
        unrealized_pnl: 0.0,
        total_trades: 0,
        grid_filled_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        started_at: None,
        stopped_at: None,
    }
}

#[test]
fn g1_1_normal_spacing() {
    let bot = make_grid_bot(50000.0, 40000.0, 10, StrategyStatus::Running, 0.0, 10000.0);
    assert!((bot.grid_spacing() - 1000.0).abs() < 0.01);
}

#[test]
fn g1_2_zero_range() {
    let bot = make_grid_bot(50000.0, 50000.0, 10, StrategyStatus::Running, 0.0, 10000.0);
    assert!((bot.grid_spacing() - 0.0).abs() < 0.01);
}

#[test]
fn g1_3_zero_grid_count() {
    let bot = make_grid_bot(50000.0, 40000.0, 0, StrategyStatus::Running, 0.0, 10000.0);
    assert!((bot.grid_spacing() - 0.0).abs() < 0.01);
}

#[test]
fn g3_1_running_status() {
    let bot = make_grid_bot(50000.0, 40000.0, 10, StrategyStatus::Running, 0.0, 10000.0);
    assert!(bot.is_running());
}

#[test]
fn g3_2_stopped_status() {
    let bot = make_grid_bot(50000.0, 40000.0, 10, StrategyStatus::Stopped, 0.0, 10000.0);
    assert!(!bot.is_running());
}

#[test]
fn g4_1_stopped_status() {
    let bot = make_grid_bot(50000.0, 40000.0, 10, StrategyStatus::Stopped, 0.0, 10000.0);
    assert!(bot.is_stopped());
}

#[test]
fn g4_2_running_status() {
    let bot = make_grid_bot(50000.0, 40000.0, 10, StrategyStatus::Running, 0.0, 10000.0);
    assert!(!bot.is_stopped());
}

#[test]
fn g5_1_positive_return() {
    let bot = make_grid_bot(
        50000.0,
        40000.0,
        10,
        StrategyStatus::Running,
        500.0,
        10000.0,
    );
    assert!((bot.total_return_pct() - 5.0).abs() < 0.01);
}

#[test]
fn g5_2_zero_return() {
    let bot = make_grid_bot(50000.0, 40000.0, 10, StrategyStatus::Running, 0.0, 10000.0);
    assert!((bot.total_return_pct() - 0.0).abs() < 0.01);
}

#[test]
fn g5_3_zero_capital_division_protection() {
    let bot = make_grid_bot(50000.0, 40000.0, 10, StrategyStatus::Running, 500.0, 0.0);
    assert!((bot.total_return_pct() - 0.0).abs() < 0.01);
}
