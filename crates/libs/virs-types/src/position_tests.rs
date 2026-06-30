//! Unit tests for position.rs methods.

use chrono::Utc;
use uuid::Uuid;

use crate::enums::*;
use crate::position::{Position, PositionOrder, RiskConfig};

// ============================================================
// TC-P1-P3: Position status methods
// ============================================================

#[test]
fn p1_1_open_is_open() {
    let pos = make_position(PositionStatus::Open);
    assert!(pos.is_open());
    assert!(!pos.is_closed());
}

#[test]
fn p2_1_closed_is_closed() {
    let pos = make_position(PositionStatus::Closed);
    assert!(pos.is_closed());
    assert!(!pos.is_open());
}

#[test]
fn p3_1_empty_is_empty() {
    let pos = make_position(PositionStatus::Empty);
    assert!(pos.is_empty());
}

// ============================================================
// TC-P4-P5: Position direction methods
// ============================================================

#[test]
fn p4_1_long_is_long() {
    let pos = make_position_with_side(PositionSide::Long);
    assert!(pos.is_long());
    assert!(!pos.is_short());
}

#[test]
fn p5_1_short_is_short() {
    let pos = make_position_with_side(PositionSide::Short);
    assert!(pos.is_short());
    assert!(!pos.is_long());
}

// ============================================================
// TC-P6: Position::unrealized_pnl_at
// ============================================================

#[test]
fn p6_1_long_pnl() {
    let mut pos = make_position_with_side(PositionSide::Long);
    pos.entry_price = 50000.0;
    pos.size = 1.0;
    assert!((pos.unrealized_pnl_at(51000.0) - 1000.0).abs() < 0.01);
}

#[test]
fn p6_2_short_pnl() {
    let mut pos = make_position_with_side(PositionSide::Short);
    pos.entry_price = 50000.0;
    pos.size = 1.0;
    assert!((pos.unrealized_pnl_at(49000.0) - 1000.0).abs() < 0.01);
}

// ============================================================
// TC-P7: Position::pnl_pct_at
// ============================================================

#[test]
fn p7_1_long_pnl_pct() {
    let mut pos = make_position_with_side(PositionSide::Long);
    pos.entry_price = 50000.0;
    pos.size = 1.0;
    pos.margin = 50000.0;
    assert!((pos.pnl_pct_at(51000.0) - 2.0).abs() < 0.01);
}

// ============================================================
// TC-P8-P11: PositionOrder methods
// ============================================================

#[test]
fn p8_1_filled_is_filled() {
    let order = make_order(OrderStatus::Filled, 10.0, 10.0);
    assert!(order.is_filled());
}

#[test]
fn p9_1_open_is_open() {
    let order = make_order(OrderStatus::Open, 0.0, 10.0);
    assert!(order.is_open());
}

#[test]
fn p9_2_partially_filled_is_open() {
    let order = make_order(OrderStatus::PartiallyFilled, 5.0, 10.0);
    assert!(order.is_open());
}

#[test]
fn p10_1_canceled_is_canceled() {
    let order = make_order(OrderStatus::Canceled, 0.0, 10.0);
    assert!(order.is_canceled());
}

#[test]
fn p11_1_half_fill_rate() {
    let order = make_order(OrderStatus::PartiallyFilled, 5.0, 10.0);
    assert!((order.fill_rate() - 0.5).abs() < 0.0001);
}

#[test]
fn p11_2_zero_amount_protection() {
    let order = make_order(OrderStatus::Open, 0.0, 0.0);
    assert!((order.fill_rate() - 0.0).abs() < 0.0001);
}

// ============================================================
// TC-P12: RiskConfig::validate
// ============================================================

#[test]
fn p12_1_default_valid() {
    let config = RiskConfig::default();
    assert!(config.validate().is_ok());
}

#[test]
fn p12_2_zero_leverage() {
    let mut config = RiskConfig::default();
    config.max_leverage = 0;
    assert!(config.validate().is_err());
}

#[test]
fn p12_3_negative_drawdown() {
    let mut config = RiskConfig::default();
    config.max_drawdown_pct = -0.1;
    assert!(config.validate().is_err());
}

#[test]
fn p12_4_negative_position_pct() {
    let mut config = RiskConfig::default();
    config.max_position_per_symbol_pct = -1.0;
    assert!(config.validate().is_err());
}

// ============================================================
// Helpers
// ============================================================

fn make_position(status: PositionStatus) -> Position {
    Position {
        id: Uuid::nil(),
        engine_id: "test".into(),
        strategy_id: None,
        exchange: "binance".into(),
        symbol: "BTC/USDT".into(),
        side: PositionSide::Long,
        status,
        size: 1.0,
        entry_price: 50000.0,
        current_price: 50000.0,
        leverage: 10,
        margin: 5000.0,
        unrealized_pnl: 0.0,
        realized_pnl: 0.0,
        stop_loss: None,
        take_profit: None,
        liquidation_price: None,
        opened_at: Utc::now(),
        updated_at: Utc::now(),
        closed_at: None,
        metadata: serde_json::json!({}),
    }
}

fn make_position_with_side(side: PositionSide) -> Position {
    let mut pos = make_position(PositionStatus::Open);
    pos.side = side;
    pos
}

fn make_order(status: OrderStatus, filled: f64, amount: f64) -> PositionOrder {
    PositionOrder {
        id: Uuid::nil(),
        position_id: Uuid::nil(),
        exchange_order_id: None,
        client_order_id: None,
        exchange: "binance".into(),
        symbol: "BTC/USDT".into(),
        side: Side::Buy,
        order_type: OrderType::Limit,
        request_price: Some(50000.0),
        fill_price: None,
        amount,
        filled,
        remaining: amount - filled,
        status,
        reduce_only: false,
        fee: 0.0,
        fee_currency: "USDT".into(),
        slippage: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}
