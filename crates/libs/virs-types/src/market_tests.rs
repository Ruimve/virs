//! Unit tests for market.rs methods.

use chrono::Utc;

use crate::enums::PositionSide;
use crate::market::{Balance, ExchangePosition, OrderBook, Ticker};

// ============================================================
// TC-M1: Balance::compute_total
// ============================================================

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

// ============================================================
// TC-M2: Ticker::mid_price
// ============================================================

#[test]
fn m2_1_mid_price() {
    let ticker = make_ticker(99.0, 101.0);
    assert!((ticker.mid_price() - 100.0).abs() < 0.01);
}

// ============================================================
// TC-M3: Ticker::spread
// ============================================================

#[test]
fn m3_1_spread() {
    let ticker = make_ticker(99.0, 101.0);
    assert!((ticker.spread() - 2.0).abs() < 0.01);
}

// ============================================================
// TC-M4-M7: OrderBook methods
// ============================================================

#[test]
fn m4_1_best_bid() {
    let ob = make_order_book(vec![(100.0, 1.0)], vec![(101.0, 1.0)]);
    assert_eq!(ob.best_bid(), Some(100.0));
}

#[test]
fn m4_2_empty_bids() {
    let ob = make_order_book(vec![], vec![(101.0, 1.0)]);
    assert_eq!(ob.best_bid(), None);
}

#[test]
fn m5_1_best_ask() {
    let ob = make_order_book(vec![(100.0, 1.0)], vec![(101.0, 1.0)]);
    assert_eq!(ob.best_ask(), Some(101.0));
}

#[test]
fn m5_2_empty_asks() {
    let ob = make_order_book(vec![(100.0, 1.0)], vec![]);
    assert_eq!(ob.best_ask(), None);
}

#[test]
fn m6_1_spread() {
    let ob = make_order_book(vec![(100.0, 1.0)], vec![(101.0, 1.0)]);
    assert!((ob.spread().unwrap() - 1.0).abs() < 0.01);
}

#[test]
fn m6_2_empty_spread() {
    let ob = make_order_book(vec![], vec![]);
    assert_eq!(ob.spread(), None);
}

#[test]
fn m7_1_mid_price() {
    let ob = make_order_book(vec![(100.0, 1.0)], vec![(102.0, 1.0)]);
    assert!((ob.mid_price().unwrap() - 101.0).abs() < 0.01);
}

// ============================================================
// TC-M8-M11: ExchangePosition methods
// ============================================================

#[test]
fn m8_1_is_long() {
    let pos = make_exchange_position(PositionSide::Long, 50000.0, 1.0);
    assert!(pos.is_long());
    assert!(!pos.is_short());
}

#[test]
fn m9_1_is_short() {
    let pos = make_exchange_position(PositionSide::Short, 50000.0, 1.0);
    assert!(pos.is_short());
    assert!(!pos.is_long());
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

#[test]
fn m11_1_pnl_pct() {
    let pos = make_exchange_position(PositionSide::Long, 50000.0, 1.0);
    assert!((pos.pnl_pct_at(51000.0) - 2.0).abs() < 0.01);
}

// ============================================================
// Helpers
// ============================================================

fn make_ticker(bid: f64, ask: f64) -> Ticker {
    Ticker {
        symbol: "BTC/USDT".into(),
        exchange: "binance".into(),
        bid, ask, last: 100.0,
        high_24h: 110.0, low_24h: 90.0,
        volume_24h: 1000.0,
        price_change_24h: 5.0,
        price_change_pct_24h: 5.0,
        timestamp: Utc::now(),
    }
}

fn make_order_book(bids: Vec<(f64, f64)>, asks: Vec<(f64, f64)>) -> OrderBook {
    OrderBook {
        symbol: "BTC/USDT".into(),
        bids, asks,
        timestamp: Utc::now(),
    }
}

fn make_exchange_position(side: PositionSide, entry: f64, size: f64) -> ExchangePosition {
    ExchangePosition {
        symbol: "BTC/USDT".into(),
        side,
        size,
        entry_price: entry,
        leverage: 10,
        unrealized_pnl: 0.0,
        liquidation_price: None,
    }
}
