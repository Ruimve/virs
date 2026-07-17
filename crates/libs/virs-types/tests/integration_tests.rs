use chrono::Utc;
use uuid::Uuid;

use virs_types::enums::*;
use virs_types::market::*;
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

#[test]
fn int_3_1_exchange_position_pnl_chain() {
    let pos = ExchangePosition {
        symbol: "BTC/USDT".into(),
        side: PositionSide::Long,
        quantity: 1.0,
        entry_price: 50000.0,
    };
    assert!((pos.unrealized_pnl_at(51000.0) - 1000.0).abs() < 0.01);
}

#[test]
fn int_8_1_exchange_position_serde_then_pnl() {
    let pos = ExchangePosition {
        symbol: "BTC/USDT".into(),
        side: PositionSide::Long,
        quantity: 2.0,
        entry_price: 50000.0,
    };
    let original_pnl = pos.unrealized_pnl_at(52000.0);
    let json = serde_json::to_string(&pos).unwrap();
    let de: ExchangePosition = serde_json::from_str(&json).unwrap();
    assert!((de.unrealized_pnl_at(52000.0) - original_pnl).abs() < 0.01);
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
        stop_loss: None,
        take_profit: None,
        client_order_id: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}
