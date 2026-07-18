use chrono::Utc;
use uuid::Uuid;

use virs_types::enums::*;
use virs_types::position::*;

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
        symbol: "BTC/USDT".into(),
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
