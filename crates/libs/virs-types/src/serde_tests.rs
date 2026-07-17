use chrono::Utc;

use crate::enums::*;
use crate::market::*;


#[test]
fn s1_1_side_roundtrip() {
    let json = serde_json::to_string(&Side::Buy).unwrap();
    let de: Side = serde_json::from_str(&json).unwrap();
    assert_eq!(de, Side::Buy);
}

#[test]
fn s1_2_order_status_roundtrip() {
    let json = serde_json::to_string(&OrderStatus::Filled).unwrap();
    let de: OrderStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(de, OrderStatus::Filled);
}

#[test]
fn s1_3_market_type_roundtrip() {
    let json = serde_json::to_string(&MarketType::Perpetual).unwrap();
    let de: MarketType = serde_json::from_str(&json).unwrap();
    assert_eq!(de, MarketType::Perpetual);
}

#[test]
fn s1_4_strategy_status_roundtrip() {
    let json = serde_json::to_string(&StrategyStatus::Running).unwrap();
    let de: StrategyStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(de, StrategyStatus::Running);
}


#[test]
fn s2_1_ticker_roundtrip() {
    let ticker = Ticker {
        symbol: "BTC/USDT".into(), exchange: "binance".into(),
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
        symbol: "BTC/USDT".into(), side: PositionSide::Long,
        quantity: 1.0, entry_price: 50000.0, leverage: 10,
    };
    let json = serde_json::to_string(&pos).unwrap();
    let de: ExchangePosition = serde_json::from_str(&json).unwrap();
    assert_eq!(de, pos);
}
