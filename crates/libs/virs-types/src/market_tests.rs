use crate::enums::PositionSide;
use crate::market::{Balance, ExchangePosition};


#[test]
fn m1_1_normal_total() {
    let balance = Balance { asset: "USDT".into(), free: 100.0, used: 50.0, total: 150.0 };
    assert!((balance.compute_total() - 150.0).abs() < 0.01);
}

#[test]
fn m1_2_zero_total() {
    let balance = Balance { asset: "USDT".into(), free: 0.0, used: 0.0, total: 0.0 };
    assert!((balance.compute_total() - 0.0).abs() < 0.01);
}


#[test]
fn m10_1_long_profit() {
    let pos = make_exchange_position(PositionSide::Long, 50000.0, 1.0);
    assert!((pos.unrealized_pnl_at(51000.0) - 1000.0).abs() < 0.01);
}

#[test]
fn m10_2_short_profit() {
    let pos = make_exchange_position(PositionSide::Short, 50000.0, 1.0);
    assert!((pos.unrealized_pnl_at(49000.0) - 1000.0).abs() < 0.01);
}

#[test]
fn m10_3_long_loss() {
    let pos = make_exchange_position(PositionSide::Long, 50000.0, 1.0);
    assert!((pos.unrealized_pnl_at(49000.0) - (-1000.0)).abs() < 0.01);
}


fn make_exchange_position(side: PositionSide, entry: f64, quantity: f64) -> ExchangePosition {
    ExchangePosition {
        symbol: "BTC/USDT".into(),
        side,
        quantity,
        entry_price: entry,
        leverage: 10,
    }
}
