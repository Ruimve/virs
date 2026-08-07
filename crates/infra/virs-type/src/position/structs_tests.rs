use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::*;
use crate::position::{PositionSide, PositionStatus};


#[test]
fn p1_1_open_is_open() {
    let pos = make_position(PositionStatus::Open);
    assert!(pos.is_open());
}


#[test]
fn p6_1_long_pnl() {
    let mut pos = make_position(PositionStatus::Open);
    pos.side = PositionSide::Long;
    pos.entry_price = 50000.0;
    pos.quantity = 1.0;
    assert!((pos.unrealized_pnl_at(51000.0) - 1000.0).abs() < 0.01);
}

#[test]
fn p6_2_short_pnl() {
    let mut pos = make_position(PositionStatus::Open);
    pos.side = PositionSide::Short;
    pos.entry_price = 50000.0;
    pos.quantity = 1.0;
    assert!((pos.unrealized_pnl_at(49000.0) - 1000.0).abs() < 0.01);
}


#[test]
fn p2_1_open_first_fill() {
    let mut pos = make_opening_long();
    let is_closed = pos.apply_fill(false, 100.0, 1.0, 0.0, ts(1000));
    assert!(!is_closed);
    assert_eq!(pos.status, PositionStatus::Open);
    assert!((pos.quantity - 1.0).abs() < 1e-10);
    assert!((pos.entry_price - 100.0).abs() < 1e-10);
    assert!((pos.realized_pnl - 0.0).abs() < 1e-10);
}

#[test]
fn p2_2_open_add_to_existing_weighted_avg() {
    let mut pos = make_open_long(1.0, 100.0);
    pos.apply_fill(false, 120.0, 1.0, 0.0, ts(2000));
    assert!((pos.quantity - 2.0).abs() < 1e-10);

    assert!((pos.entry_price - 110.0).abs() < 1e-10);
}

#[test]
fn p2_3_open_zero_fill_price_does_not_update_entry() {
    let mut pos = make_open_long(1.0, 100.0);
    pos.apply_fill(false, 0.0, 0.5, 0.0, ts(2000));

    assert!((pos.quantity - 1.5).abs() < 1e-10);
    assert!((pos.entry_price - 100.0).abs() < 1e-10);
}


#[test]
fn p2_4_close_partial_entry_unchanged() {
    let mut pos = make_open_long(2.0, 110.0);
    let is_closed = pos.apply_fill(true, 0.0, 0.5, 50.0, ts(2000));
    assert!(!is_closed);
    assert_eq!(pos.status, PositionStatus::Open);
    assert!((pos.quantity - 1.5).abs() < 1e-10);
    assert!((pos.entry_price - 110.0).abs() < 1e-10);
    assert!((pos.realized_pnl - 50.0).abs() < 1e-10);
}

#[test]
fn p2_5_close_full_returns_closed() {
    let mut pos = make_open_long(1.0, 100.0);
    let is_closed = pos.apply_fill(true, 0.0, 1.0, 100.0, ts(2000));
    assert!(is_closed);
    assert_eq!(pos.status, PositionStatus::Closed);
    assert!((pos.quantity - 0.0).abs() < 1e-10);
    assert!((pos.entry_price - 100.0).abs() < 1e-10);
    assert!((pos.realized_pnl - 100.0).abs() < 1e-10);
}

#[test]
fn p2_6_close_with_zero_fill_price_still_updates() {


    let mut pos = make_open_long(1.0, 100.0);
    let is_closed = pos.apply_fill(true, 0.0, 0.5, 50.0, ts(2000));
    assert!(!is_closed);
    assert!((pos.quantity - 0.5).abs() < 1e-10);
    assert!((pos.realized_pnl - 50.0).abs() < 1e-10);
}


#[test]
fn p2_7_open_after_partial_close_marginal_cost() {

    let mut pos = make_opening_long();


    pos.apply_fill(false, 100.0, 2.0, 0.0, ts(1000));
    assert!((pos.entry_price - 100.0).abs() < 1e-10);


    pos.apply_fill(true, 0.0, 1.0, 50.0, ts(2000));
    assert!((pos.quantity - 1.0).abs() < 1e-10);
    assert!((pos.entry_price - 100.0).abs() < 1e-10);


    pos.apply_fill(false, 120.0, 1.0, 0.0, ts(3000));
    assert!((pos.quantity - 2.0).abs() < 1e-10);

    assert!((pos.entry_price - 110.0).abs() < 1e-10);


}

#[test]
fn p2_8_full_replay_sequence_matches_runtime() {

    let mut pos = make_opening_long();


    pos.apply_fill(false, 100.0, 2.0, 0.0, ts(1000));
    let is_closed = pos.apply_fill(true, 0.0, 2.0, 200.0, ts(2000));
    assert!(is_closed);
    assert_eq!(pos.status, PositionStatus::Closed);
    assert!((pos.realized_pnl - 200.0).abs() < 1e-10);


    pos.apply_fill(false, 110.0, 1.0, 0.0, ts(3000));
    assert_eq!(pos.status, PositionStatus::Open);
    assert!((pos.quantity - 1.0).abs() < 1e-10);
    assert!((pos.entry_price - 110.0).abs() < 1e-10);

    assert!((pos.realized_pnl - 200.0).abs() < 1e-10);
}


#[test]
fn p3_1_new_for_replay_initial_state() {
    let created = ts(5000);
    let pos = Position::new_for_replay(
        "binance",
        "BTC/USDT",
        PositionSide::Long,
        Some("CL123".into()),
        created,
    );
    assert_eq!(pos.status, PositionStatus::Opening);
    assert!((pos.quantity - 0.0).abs() < 1e-10);
    assert!((pos.entry_price - 0.0).abs() < 1e-10);
    assert!((pos.realized_pnl - 0.0).abs() < 1e-10);
    assert_eq!(pos.client_order_id.as_deref(), Some("CL123"));
    assert_eq!(pos.created_at, created);
    assert_eq!(pos.updated_at, created);
    assert_eq!(pos.side, PositionSide::Long);
}

#[test]
fn p3_2_new_for_replay_then_apply_fill_matches_runtime() {

    let created = ts(1000);
    let mut replay_pos = Position::new_for_replay(
        "binance",
        "BTC/USDT",
        PositionSide::Long,
        Some("CL001".into()),
        created,
    );
    replay_pos.apply_fill(false, 100.0, 1.0, 0.0, ts(1000));
    replay_pos.apply_fill(false, 110.0, 1.0, 0.0, ts(2000));
    replay_pos.apply_fill(true, 0.0, 0.5, 25.0, ts(3000));


    let expected = Position {
        id: replay_pos.id,
        exchange: "binance".into(),
        symbol: "BTC/USDT".into(),
        side: PositionSide::Long,
        status: PositionStatus::Open,
        quantity: 1.5,
        entry_price: 105.0,
        realized_pnl: 25.0,
        client_order_id: Some("CL001".into()),
        created_at: created,
        updated_at: ts(3000),
    };
    assert_eq!(replay_pos, expected);
}


#[test]
fn p4_1_uuid_deterministic_same_inputs() {
    let a = position_uuid_v5("binance", "BTC/USDT", &PositionSide::Long);
    let b = position_uuid_v5("binance", "BTC/USDT", &PositionSide::Long);
    assert_eq!(a, b);
}

#[test]
fn p4_2_uuid_differs_by_side() {
    let long = position_uuid_v5("binance", "BTC/USDT", &PositionSide::Long);
    let short = position_uuid_v5("binance", "BTC/USDT", &PositionSide::Short);
    assert_ne!(long, short);
}

#[test]
fn p4_3_uuid_differs_by_symbol() {
    let btc = position_uuid_v5("binance", "BTC/USDT", &PositionSide::Long);
    let eth = position_uuid_v5("binance", "ETH/USDT", &PositionSide::Long);
    assert_ne!(btc, eth);
}


fn ts(millis: i64) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(millis).unwrap()
}

fn make_position(status: PositionStatus) -> Position {
    Position {
        id: Uuid::nil(),
        exchange: "binance".into(),
        symbol: "BTC/USDT".into(),
        side: PositionSide::Long,
        status,
        quantity: 1.0,
        entry_price: 50000.0,
        realized_pnl: 0.0,
        client_order_id: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

fn make_opening_long() -> Position {
    Position::new_for_replay(
        "binance",
        "BTC/USDT",
        PositionSide::Long,
        None,
        Utc::now(),
    )
}

fn make_open_long(qty: f64, entry: f64) -> Position {
    let mut pos = make_opening_long();
    pos.apply_fill(false, entry, qty, 0.0, Utc::now());
    pos
}
