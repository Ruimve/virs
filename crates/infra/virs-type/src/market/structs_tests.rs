use chrono::Utc;

use super::*;
use crate::exchange::MarginMode;
use crate::position::PositionSide;


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
fn s2_1_ticker_roundtrip() {
    let ticker = Ticker {
        symbol: "BTCUSDT".into(), exchange: "binance".into(),
        bid: Some(99.0), ask: Some(101.0), last: 100.0,
        high_24h: 110.0, low_24h: 90.0, volume_24h: 1000.0,
        price_change_24h: 5.0, price_change_pct_24h: 5.0,
        timestamp: Utc::now(),
    };
    let json = serde_json::to_string(&ticker).unwrap();
    let de: Ticker = serde_json::from_str(&json).unwrap();
    assert_eq!(de, ticker);
}

#[test]
fn s2_2_balance_roundtrip() {
    let balance = Balance { asset: "USDT".into(), free: 100.0, used: 50.0, total: 150.0 };
    let json = serde_json::to_string(&balance).unwrap();
    let de: Balance = serde_json::from_str(&json).unwrap();
    assert_eq!(de, balance);
}

#[test]
fn s2_3_exchange_position_roundtrip() {
    let pos = ExchangePosition {
        symbol: "BTCUSDT".into(), side: PositionSide::Long,
        quantity: 1.0, entry_price: 50000.0,
        margin_mode: MarginMode::Cross,
        info: serde_json::json!({}),
    };
    let json = serde_json::to_string(&pos).unwrap();
    let de: ExchangePosition = serde_json::from_str(&json).unwrap();
    assert_eq!(de, pos);
}
