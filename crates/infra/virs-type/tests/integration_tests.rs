use chrono::Utc;
use uuid::Uuid;

use virs_type::AutoBot;
use virs_type::*;
use virs_type::*;

#[test]
fn int_1_1_long_position_pnl_chain() {
    let pos = make_position(PositionSide::Long, 50000.0, 1.0);
    let pnl = pos.unrealized_pnl_at(51000.0);
    assert!((pnl - 1000.0).abs() < 0.01);
}

#[test]
fn int_1_2_short_position_pnl_chain() {
    let pos = make_position(PositionSide::Short, 50000.0, 1.0);
    let pnl = pos.unrealized_pnl_at(49000.0);
    assert!((pnl - 1000.0).abs() < 0.01);
}

fn make_position(side: PositionSide, entry: f64, quantity: f64) -> Position {
    Position {
        id: Uuid::nil(),
        exchange: "binance".into(),
        symbol: "BTCUSDT".into(),
        side,
        status: PositionStatus::Open,
        quantity,
        entry_price: entry,
        realized_pnl: 0.0,
        client_order_id: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}


fn make_auto_bot(
    status: &str,
    total_trades: i32,
    win_trades: i32,
    loss_trades: i32,
    total_pnl: f64,
) -> AutoBot {
    AutoBot {
        id: Uuid::nil(),
        user_id: Uuid::nil(),
        name: "stats_bot".into(),
        symbol: "BTCUSDT".into(),
        exchange: "binance".into(),
        paper_mode: false,
        status: status.into(),
        leverage: 10,
        max_position_pct: 100.0,
        decide_interval_secs: 3600,
        initial_capital: 10000.0,
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
fn int_3_1_win_plus_loss_equals_100() {
    let bot = make_auto_bot("running", 20, 14, 6, 500.0);
    let win = bot.win_rate();
    let loss = bot.loss_rate();
    assert!((win + loss - 100.0).abs() < 0.01);
    assert!((win - 70.0).abs() < 0.01);
    assert!((loss - 30.0).abs() < 0.01);
    assert!((bot.total_return_pct() - 5.0).abs() < 0.01);
    assert!(bot.is_running());
}

#[test]
fn int_3_2_negative_return() {
    let bot = make_auto_bot("stopped", 10, 3, 7, -1500.0);
    assert!((bot.total_return_pct() - (-15.0)).abs() < 0.01);
    assert!((bot.win_rate() - 30.0).abs() < 0.01);
    assert!(bot.is_stopped());
    assert!(!bot.is_running());
}

#[test]
fn int_5_2_auto_bot_serde_then_win_rate() {
    let bot = make_auto_bot("running", 25, 15, 10, 800.0);
    let original_win_rate = bot.win_rate();
    let json = serde_json::to_string(&bot).unwrap();
    let de: AutoBot = serde_json::from_str(&json).unwrap();
    assert!((de.win_rate() - original_win_rate).abs() < 0.01);
    assert!(de.is_running());
}
