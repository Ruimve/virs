use chrono::Utc;
use uuid::Uuid;

use crate::enums::*;
use crate::position::Position;


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
        stop_loss: None,
        take_profit: None,
        client_order_id: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        closed_at: None,
    }
}
