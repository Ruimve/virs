use chrono::Utc;
use uuid::Uuid;

use super::Bot;

fn make_bot(
    status: &str,
    total_trades: i32,
    win_trades: i32,
    loss_trades: i32,
    total_pnl: f64,
    initial_capital: f64,
) -> Bot {
    Bot {
        id: Uuid::nil(),
        user_id: Uuid::nil(),
        name: "test_bot".into(),
        symbol: "BTCUSDT".into(),
        exchange: "binance".into(),
        paper_mode: false,
        status: status.into(),
        bot_type: "chat".into(),
        leverage: 10,
        max_position_pct: 100.0,
        decide_interval_secs: 3600,
        initial_capital,
        position_id_long: None,
        position_id_short: None,
        market_regime: None,
        ai_analysis: None,
        system_prompt: None,
        user_prompt: None,
        total_pnl,
        total_trades,
        win_trades,
        loss_trades,
        last_decided_at: None,
        strategy_file: None,
        auto_optimize_enabled: false,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        started_at: None,
        stopped_at: None,
    }
}

#[test]
fn a1_1_seventy_percent() {
    let bot = make_bot("running", 10, 7, 3, 0.0, 10000.0);
    assert!((bot.win_rate() - 70.0).abs() < 0.01);
}

#[test]
fn a1_2_zero_wins() {
    let bot = make_bot("running", 10, 0, 10, 0.0, 10000.0);
    assert!((bot.win_rate() - 0.0).abs() < 0.01);
}

#[test]
fn a1_3_zero_trades_division_protection() {
    let bot = make_bot("running", 0, 0, 0, 0.0, 10000.0);
    assert!((bot.win_rate() - 0.0).abs() < 0.01);
}

#[test]
fn a2_1_thirty_percent() {
    let bot = make_bot("running", 10, 7, 3, 0.0, 10000.0);
    assert!((bot.loss_rate() - 30.0).abs() < 0.01);
}

#[test]
fn a2_2_zero_trades_division_protection() {
    let bot = make_bot("running", 0, 0, 0, 0.0, 10000.0);
    assert!((bot.loss_rate() - 0.0).abs() < 0.01);
}

#[test]
fn a3_1_positive_return() {
    let bot = make_bot("running", 10, 7, 3, 1000.0, 10000.0);
    assert!((bot.total_return_pct() - 10.0).abs() < 0.01);
}

#[test]
fn a3_2_negative_return() {
    let bot = make_bot("running", 10, 7, 3, -500.0, 10000.0);
    assert!((bot.total_return_pct() - (-5.0)).abs() < 0.01);
}

#[test]
fn a3_3_zero_capital_division_protection() {
    let bot = make_bot("running", 10, 7, 3, 1000.0, 0.0);
    assert!((bot.total_return_pct() - 0.0).abs() < 0.01);
}

#[test]
fn a4_1_running_status() {
    let bot = make_bot("running", 0, 0, 0, 0.0, 10000.0);
    assert!(bot.is_running());
}

#[test]
fn a4_2_stopped_status() {
    let bot = make_bot("stopped", 0, 0, 0, 0.0, 10000.0);
    assert!(!bot.is_running());
}

#[test]
fn a5_1_stopped_status() {
    let bot = make_bot("stopped", 0, 0, 0, 0.0, 10000.0);
    assert!(bot.is_stopped());
}

#[test]
fn a5_2_running_status() {
    let bot = make_bot("running", 0, 0, 0, 0.0, 10000.0);
    assert!(!bot.is_stopped());
}

#[test]
fn s4_1_bot_roundtrip() {
    let now = Utc::now();
    let bot = Bot {
        id: Uuid::nil(),
        user_id: Uuid::nil(),
        name: "test_bot".into(),
        symbol: "ETHUSDT".into(),
        exchange: "binance".into(),
        paper_mode: false,
        status: "running".into(),
        bot_type: "chat".into(),
        leverage: 5,
        max_position_pct: 80.0,
        decide_interval_secs: 1800,
        initial_capital: 5000.0,
        position_id_long: None,
        position_id_short: None,
        market_regime: None,
        ai_analysis: None,
        system_prompt: None,
        user_prompt: None,
        total_pnl: 250.0,
        total_trades: 15,
        win_trades: 10,
        loss_trades: 5,
        last_decided_at: Some(now),
        strategy_file: None,
        auto_optimize_enabled: false,
        created_at: now,
        updated_at: now,
        started_at: Some(now),
        stopped_at: None,
    };
    let json = serde_json::to_string(&bot).unwrap();
    let de: Bot = serde_json::from_str(&json).unwrap();
    assert_eq!(de.win_trades, bot.win_trades);
    assert_eq!(de.total_pnl, bot.total_pnl);
    assert_eq!(de.status, bot.status);
}
