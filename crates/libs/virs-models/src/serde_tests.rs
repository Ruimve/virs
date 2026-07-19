use chrono::Utc;
use uuid::Uuid;

use virs_types::enums::*;

use crate::{AutoBot, GridBot, Order, StrategyStatus};

#[test]
fn s1_1_order_roundtrip() {
    let now = Utc::now();
    let order = Order {
        id: "order_123".into(),
        client_order_id: Some("client_456".into()),
        symbol: "BTC/USDT".into(),
        side: Side::Buy,
        order_type: OrderType::Limit,
        price: Some(50000.0),
        amount: 1.0,
        cost: Some(50000.0),
        filled: 0.5,
        remaining: 0.5,
        status: OrderStatus::PartiallyFilled,
        fee: 0.075,
        fee_currency: "BTC".into(),
        created_at: now,
        updated_at: now,
    };
    let json = serde_json::to_string(&order).unwrap();
    let de: Order = serde_json::from_str(&json).unwrap();
    assert_eq!(de, order);
}

#[test]
fn s3_1_grid_bot_roundtrip() {
    let now = Utc::now();
    let bot = GridBot {
        id: Uuid::nil(),
        user_id: Uuid::nil(),
        name: "grid_bot".into(),
        symbol: "BTC/USDT".into(),
        exchange: "binance".into(),
        paper_mode: true,
        status: StrategyStatus::Running,
        upper_price: 50000.0,
        lower_price: 40000.0,
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
        total_pnl: 500.0,
        unrealized_pnl: 100.0,
        total_trades: 20,
        grid_filled_count: 5,
        created_at: now,
        updated_at: now,
        started_at: Some(now),
        stopped_at: None,
    };
    let json = serde_json::to_string(&bot).unwrap();
    let de: GridBot = serde_json::from_str(&json).unwrap();
    assert_eq!(de.id, bot.id);
    assert_eq!(de.upper_price, bot.upper_price);
    assert_eq!(de.grid_count, bot.grid_count);
    assert_eq!(de.status, bot.status);
    assert_eq!(de.total_pnl, bot.total_pnl);
}

#[test]
fn s4_1_auto_bot_roundtrip() {
    let now = Utc::now();
    let bot = AutoBot {
        id: Uuid::nil(),
        user_id: Uuid::nil(),
        name: "auto_bot".into(),
        symbol: "ETH/USDT".into(),
        exchange: "binance".into(),
        paper_mode: false,
        status: "running".into(),
        leverage: 5,
        max_position_pct: 80.0,
        decide_interval_secs: 1800,
        initial_capital: 5000.0,
        position_id: None,
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
        created_at: now,
        updated_at: now,
        started_at: Some(now),
        stopped_at: None,
    };
    let json = serde_json::to_string(&bot).unwrap();
    let de: AutoBot = serde_json::from_str(&json).unwrap();
    assert_eq!(de.win_trades, bot.win_trades);
    assert_eq!(de.total_pnl, bot.total_pnl);
    assert_eq!(de.status, bot.status);
}
