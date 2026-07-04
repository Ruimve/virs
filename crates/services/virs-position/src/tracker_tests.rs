//! Unit tests for tracker.rs idempotent functions and snapshot

use std::collections::HashMap;

use crate::tracker::{calc_drawdown_pct, calc_unrealized_pnl, PnlTracker};
use chrono::Utc;
use uuid::Uuid;
use virs_types::enums::{PositionSide, PositionStatus, Side, TradeType};
use virs_types::position::{Position, Trade};

fn make_position(
    symbol: &str,
    side: PositionSide,
    size: f64,
    entry_price: f64,
    current_price: f64,
) -> Position {
    Position {
        id: Uuid::new_v4(),
        engine_id: "test".to_string(),
        strategy_id: None,
        exchange: "binance".to_string(),
        symbol: symbol.to_string(),
        side,
        status: PositionStatus::Open,
        size,
        entry_price,
        current_price,
        leverage: 10,
        margin: size * entry_price / 10.0,
        unrealized_pnl: 0.0,
        realized_pnl: 0.0,
        stop_loss: None,
        take_profit: None,
        liquidation_price: None,
        opened_at: Utc::now(),
        updated_at: Utc::now(),
        closed_at: None,
        metadata: serde_json::Value::Null,
    }
}

fn make_trade(pnl: f64, trade_type: TradeType) -> Trade {
    Trade {
        id: Uuid::new_v4(),
        position_id: Uuid::new_v4(),
        order_id: Uuid::new_v4(),
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side: Side::Buy,
        price: 100.0,
        amount: 1.0,
        fee: 0.1,
        fee_currency: "USDT".to_string(),
        pnl,
        trade_type,
        created_at: Utc::now(),
    }
}

// ── calc_unrealized_pnl ────────────────────────────────────

#[test]
fn p1_1_calc_unrealized_pnl_empty() {
    let positions: Vec<&Position> = vec![];
    let prices = HashMap::new();
    let result = calc_unrealized_pnl(&positions, &prices);
    assert!((result - 0.0).abs() < 1e-10);
}

#[test]
fn p1_2_calc_unrealized_pnl_long_profit() {
    let pos = make_position("BTC/USDT", PositionSide::Long, 2.0, 100.0, 0.0);
    let positions: Vec<&Position> = vec![&pos];
    let mut prices = HashMap::new();
    prices.insert("BTC/USDT".to_string(), 110.0);
    // (110 - 100) * 2 = 20
    let result = calc_unrealized_pnl(&positions, &prices);
    assert!((result - 20.0).abs() < 1e-10);
}

#[test]
fn p1_3_calc_unrealized_pnl_long_loss() {
    let pos = make_position("BTC/USDT", PositionSide::Long, 2.0, 100.0, 0.0);
    let positions: Vec<&Position> = vec![&pos];
    let mut prices = HashMap::new();
    prices.insert("BTC/USDT".to_string(), 90.0);
    // (90 - 100) * 2 = -20
    let result = calc_unrealized_pnl(&positions, &prices);
    assert!((result - (-20.0)).abs() < 1e-10);
}

#[test]
fn p1_4_calc_unrealized_pnl_short_profit() {
    let pos = make_position("BTC/USDT", PositionSide::Short, 2.0, 100.0, 0.0);
    let positions: Vec<&Position> = vec![&pos];
    let mut prices = HashMap::new();
    prices.insert("BTC/USDT".to_string(), 90.0);
    // (100 - 90) * 2 = 20
    let result = calc_unrealized_pnl(&positions, &prices);
    assert!((result - 20.0).abs() < 1e-10);
}

#[test]
fn p1_5_calc_unrealized_pnl_short_loss() {
    let pos = make_position("BTC/USDT", PositionSide::Short, 2.0, 100.0, 0.0);
    let positions: Vec<&Position> = vec![&pos];
    let mut prices = HashMap::new();
    prices.insert("BTC/USDT".to_string(), 110.0);
    // (100 - 110) * 2 = -20
    let result = calc_unrealized_pnl(&positions, &prices);
    assert!((result - (-20.0)).abs() < 1e-10);
}

#[test]
fn p1_7_calc_unrealized_pnl_no_price() {
    let pos = make_position("BTC/USDT", PositionSide::Long, 2.0, 100.0, 105.0);
    let positions: Vec<&Position> = vec![&pos];
    let prices = HashMap::new();
    // No price in map → uses pos.current_price = 105
    // (105 - 100) * 2 = 10
    let result = calc_unrealized_pnl(&positions, &prices);
    assert!((result - 10.0).abs() < 1e-10);
}

// ── calc_drawdown_pct ──────────────────────────────────────

#[test]
fn p2_1_calc_drawdown_pct_zero_peak() {
    let result = calc_drawdown_pct(0.0, 50.0);
    assert!((result - 0.0).abs() < 1e-10);
}

#[test]
fn p2_2_calc_drawdown_pct_no_drawdown() {
    // equity > peak → no drawdown (negative value clamped to 0 conceptually)
    let result = calc_drawdown_pct(100.0, 110.0);
    // (100 - 110) / 100 = -0.1 (negative, but that's the formula)
    assert!(result < 0.0);
}

#[test]
fn p2_3_calc_drawdown_pct_partial() {
    // 10% drawdown
    let result = calc_drawdown_pct(100.0, 90.0);
    assert!((result - 0.1).abs() < 1e-10);
}

#[test]
fn p2_4_calc_drawdown_pct_full() {
    // 100% drawdown
    let result = calc_drawdown_pct(100.0, 0.0);
    assert!((result - 1.0).abs() < 1e-10);
}

// ── PnlTracker::snapshot ───────────────────────────────────

#[test]
fn p3_1_snapshot_equity_and_drawdown() {
    let mut tracker = PnlTracker::new(10000.0);
    tracker.record_trade(&make_trade(100.0, TradeType::Close));
    // peak updated to 10100 after first trade
    tracker.record_trade(&make_trade(-50.0, TradeType::Close));
    // realized = 50, equity = 10000 + 50 + 0 = 10050, peak = 10100
    let snapshot = tracker.snapshot(0.0);
    assert!((snapshot.equity - 10050.0).abs() < 1e-10);
    // drawdown = (10100 - 10050) / 10100 ≈ 0.00495
    assert!(snapshot.max_drawdown > 0.0);
}

#[test]
fn p3_2_snapshot_with_unrealized() {
    let mut tracker = PnlTracker::new(10000.0);
    tracker.record_trade(&make_trade(500.0, TradeType::Close));
    // realized = 500, unrealized = 10 → equity = 10510
    let snapshot = tracker.snapshot(10.0);
    assert!((snapshot.equity - 10510.0).abs() < 1e-10);
}
