//! Unit tests for trading.rs Order methods.

use chrono::Utc;

use virs_types::enums::*;

use crate::Order;

fn make_order(status: OrderStatus, filled: f64, amount: f64, side: Side) -> Order {
    let now = Utc::now();
    Order {
        id: "test_order".into(),
        client_order_id: None,
        symbol: "BTC/USDT".into(),
        side,
        order_type: OrderType::Limit,
        price: Some(50000.0),
        amount,
        cost: Some(filled * 50000.0),
        filled,
        remaining: amount - filled,
        status,
        fee: 0.0,
        fee_currency: "USDT".into(),
        created_at: now,
        updated_at: now,
    }
}

// ============================================================
// TC-T1: is_filled
// ============================================================

#[test]
fn t1_1_filled_status() {
    let order = make_order(OrderStatus::Filled, 1.0, 1.0, Side::Buy);
    assert!(order.is_filled());
}

#[test]
fn t1_2_partially_filled_status() {
    let order = make_order(OrderStatus::PartiallyFilled, 0.5, 1.0, Side::Buy);
    assert!(!order.is_filled());
}

#[test]
fn t1_3_open_status() {
    let order = make_order(OrderStatus::Open, 0.0, 1.0, Side::Buy);
    assert!(!order.is_filled());
}

// ============================================================
// TC-T2: is_open
// ============================================================

#[test]
fn t2_1_open_status() {
    let order = make_order(OrderStatus::Open, 0.0, 1.0, Side::Buy);
    assert!(order.is_open());
}

#[test]
fn t2_2_partially_filled_status() {
    let order = make_order(OrderStatus::PartiallyFilled, 0.5, 1.0, Side::Buy);
    assert!(order.is_open());
}

#[test]
fn t2_3_filled_status() {
    let order = make_order(OrderStatus::Filled, 1.0, 1.0, Side::Buy);
    assert!(!order.is_open());
}

#[test]
fn t2_4_canceled_status() {
    let order = make_order(OrderStatus::Canceled, 0.0, 1.0, Side::Buy);
    assert!(!order.is_open());
}

// ============================================================
// TC-T3: is_canceled
// ============================================================

#[test]
fn t3_1_canceled_status() {
    let order = make_order(OrderStatus::Canceled, 0.0, 1.0, Side::Buy);
    assert!(order.is_canceled());
}

#[test]
fn t3_2_open_status() {
    let order = make_order(OrderStatus::Open, 0.0, 1.0, Side::Buy);
    assert!(!order.is_canceled());
}

// ============================================================
// TC-T4: fill_rate
// ============================================================

#[test]
fn t4_1_half_filled() {
    let order = make_order(OrderStatus::PartiallyFilled, 5.0, 10.0, Side::Buy);
    assert!((order.fill_rate() - 0.5).abs() < 0.0001);
}

#[test]
fn t4_2_zero_filled() {
    let order = make_order(OrderStatus::Open, 0.0, 10.0, Side::Buy);
    assert!((order.fill_rate() - 0.0).abs() < 0.0001);
}

#[test]
fn t4_3_full_filled() {
    let order = make_order(OrderStatus::Filled, 10.0, 10.0, Side::Buy);
    assert!((order.fill_rate() - 1.0).abs() < 0.0001);
}

#[test]
fn t4_4_zero_amount_division_protection() {
    let order = make_order(OrderStatus::Open, 10.0, 0.0, Side::Buy);
    assert!((order.fill_rate() - 0.0).abs() < 0.0001);
}

// ============================================================
// TC-T5: is_buy
// ============================================================

#[test]
fn t5_1_buy_side() {
    let order = make_order(OrderStatus::Open, 0.0, 1.0, Side::Buy);
    assert!(order.is_buy());
}

#[test]
fn t5_2_sell_side() {
    let order = make_order(OrderStatus::Open, 0.0, 1.0, Side::Sell);
    assert!(!order.is_buy());
}

// ============================================================
// TC-T6: is_sell
// ============================================================

#[test]
fn t6_1_sell_side() {
    let order = make_order(OrderStatus::Open, 0.0, 1.0, Side::Sell);
    assert!(order.is_sell());
}

#[test]
fn t6_2_buy_side() {
    let order = make_order(OrderStatus::Open, 0.0, 1.0, Side::Buy);
    assert!(!order.is_sell());
}
