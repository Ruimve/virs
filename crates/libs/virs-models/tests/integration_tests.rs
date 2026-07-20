use chrono::Utc;
use uuid::Uuid;

use virs_models::*;

#[test]
fn int_2_1_spacing_and_return_pct() {
    let now = Utc::now();
    let bot = GridBot {
        id: Uuid::nil(),
        user_id: Uuid::nil(),
        name: "test".into(),
        symbol: "BTC/USDT".into(),
        exchange: "binance".into(),
        paper_mode: false,
        status: StrategyStatus::Running,
        upper_price: 60000.0,
        lower_price: 40000.0,
        grid_count: 20,
        grid_profit_pct: 0.5,
        quantity_per_grid: 0.01,
        leverage: 10,
        initial_capital: 10000.0,
        market_regime: None,
        ai_analysis: None,
        grid_levels_json: None,
        system_prompt: None,
        user_prompt: None,
        dynamic_adjust: false,
        adjust_interval_secs: 3600,
        last_adjusted_at: None,
        strategy_file: None,
        total_pnl: 500.0,
        unrealized_pnl: 0.0,
        total_trades: 10,
        grid_filled_count: 5,
        created_at: now,
        updated_at: now,
        started_at: None,
        stopped_at: None,
    };
    assert!((bot.grid_spacing() - 1000.0).abs() < 0.01);
    assert!(bot.is_running());
    assert!(!bot.is_stopped());
    assert!((bot.total_return_pct() - 5.0).abs() < 0.01);
}

#[test]
fn int_2_2_invalid_config_negative_spacing() {
    let now = Utc::now();
    let bot = GridBot {
        id: Uuid::nil(),
        user_id: Uuid::nil(),
        name: "invalid".into(),
        symbol: "BTC/USDT".into(),
        exchange: "binance".into(),
        paper_mode: false,
        status: StrategyStatus::Stopped,
        upper_price: 40000.0,
        lower_price: 50000.0,
        grid_count: 10,
        grid_profit_pct: 0.5,
        quantity_per_grid: 0.01,
        leverage: 10,
        initial_capital: 10000.0,
        market_regime: None,
        ai_analysis: None,
        grid_levels_json: None,
        system_prompt: None,
        user_prompt: None,
        dynamic_adjust: false,
        adjust_interval_secs: 3600,
        last_adjusted_at: None,
        strategy_file: None,
        total_pnl: 0.0,
        unrealized_pnl: 0.0,
        total_trades: 0,
        grid_filled_count: 0,
        created_at: now,
        updated_at: now,
        started_at: None,
        stopped_at: None,
    };

    assert!(bot.grid_spacing() < 0.0);
    assert!(!bot.is_running());
    assert!(bot.is_stopped());
}

#[test]
fn int_3_1_win_plus_loss_equals_100() {
    let now = Utc::now();
    let bot = AutoBot {
        id: Uuid::nil(),
        user_id: Uuid::nil(),
        name: "stats_bot".into(),
        symbol: "BTC/USDT".into(),
        exchange: "binance".into(),
        paper_mode: false,
        status: "running".into(),
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
        total_pnl: 500.0,
        total_trades: 20,
        win_trades: 14,
        loss_trades: 6,
        last_decided_at: None,
        strategy_file: None,
        created_at: now,
        updated_at: now,
        started_at: None,
        stopped_at: None,
    };
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
    let now = Utc::now();
    let bot = AutoBot {
        id: Uuid::nil(),
        user_id: Uuid::nil(),
        name: "loss_bot".into(),
        symbol: "BTC/USDT".into(),
        exchange: "binance".into(),
        paper_mode: false,
        status: "stopped".into(),
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
        total_pnl: -1500.0,
        total_trades: 10,
        win_trades: 3,
        loss_trades: 7,
        last_decided_at: None,
        strategy_file: None,
        created_at: now,
        updated_at: now,
        started_at: None,
        stopped_at: None,
    };
    assert!((bot.total_return_pct() - (-15.0)).abs() < 0.01);
    assert!((bot.win_rate() - 30.0).abs() < 0.01);
    assert!(bot.is_stopped());
    assert!(!bot.is_running());
}

#[test]
fn int_5_1_grid_bot_serde_then_methods() {
    let now = Utc::now();
    let bot = GridBot {
        id: Uuid::nil(),
        user_id: Uuid::nil(),
        name: "serde_bot".into(),
        symbol: "BTC/USDT".into(),
        exchange: "binance".into(),
        paper_mode: false,
        status: StrategyStatus::Running,
        upper_price: 50000.0,
        lower_price: 30000.0,
        grid_count: 10,
        grid_profit_pct: 0.5,
        quantity_per_grid: 0.01,
        leverage: 10,
        initial_capital: 5000.0,
        market_regime: None,
        ai_analysis: None,
        grid_levels_json: None,
        system_prompt: None,
        user_prompt: None,
        dynamic_adjust: false,
        adjust_interval_secs: 3600,
        last_adjusted_at: None,
        strategy_file: None,
        total_pnl: 250.0,
        unrealized_pnl: 0.0,
        total_trades: 5,
        grid_filled_count: 2,
        created_at: now,
        updated_at: now,
        started_at: None,
        stopped_at: None,
    };
    let original_spacing = bot.grid_spacing();
    let json = serde_json::to_string(&bot).unwrap();
    let de: GridBot = serde_json::from_str(&json).unwrap();
    assert!((de.grid_spacing() - original_spacing).abs() < 0.01);
    assert!(de.is_running());
}

#[test]
fn int_5_2_auto_bot_serde_then_win_rate() {
    let now = Utc::now();
    let bot = AutoBot {
        id: Uuid::nil(),
        user_id: Uuid::nil(),
        name: "serde_auto".into(),
        symbol: "BTC/USDT".into(),
        exchange: "binance".into(),
        paper_mode: false,
        status: "running".into(),
        leverage: 5,
        max_position_pct: 80.0,
        decide_interval_secs: 1800,
        initial_capital: 10000.0,
        position_id_long: None,
        position_id_short: None,
        market_regime: None,
        ai_analysis: None,
        system_prompt: None,
        user_prompt: None,
        total_pnl: 800.0,
        total_trades: 25,
        win_trades: 15,
        loss_trades: 10,
        last_decided_at: None,
        strategy_file: None,
        created_at: now,
        updated_at: now,
        started_at: None,
        stopped_at: None,
    };
    let original_win_rate = bot.win_rate();
    let json = serde_json::to_string(&bot).unwrap();
    let de: AutoBot = serde_json::from_str(&json).unwrap();
    assert!((de.win_rate() - original_win_rate).abs() < 0.01);
    assert!(de.is_running());
}
