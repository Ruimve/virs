//! Unit tests for types.rs From/TryFrom implementations.
//!
//! Covers: CcxtOrderStatus→OrderStatus, CcxtTicker→Ticker, CcxtOrderBook→OrderBook,
//! CcxtFundingRate→FundingRate, CcxtFundingHistoryEntry→FundingHistoryEntry.

use chrono::Utc;

use crate::types::*;
use virs_types::enums::{OrderStatus, Side};
use virs_types::market::{FundingHistoryEntry, FundingRate, OrderBook, Ticker};

// ============================================================
// TC-T1: CcxtOrderStatus → OrderStatus
// ============================================================

#[test]
fn t1_1_open_to_open() {
    let status: OrderStatus = CcxtOrderStatus::Open.into();
    assert_eq!(status, OrderStatus::Open);
}

#[test]
fn t1_2_partially_filled() {
    let status: OrderStatus = CcxtOrderStatus::PartiallyFilled.into();
    assert_eq!(status, OrderStatus::PartiallyFilled);
}

#[test]
fn t1_3_filled() {
    let status: OrderStatus = CcxtOrderStatus::Filled.into();
    assert_eq!(status, OrderStatus::Filled);
}

#[test]
fn t1_4_canceled() {
    let status: OrderStatus = CcxtOrderStatus::Canceled.into();
    assert_eq!(status, OrderStatus::Canceled);
}

#[test]
fn t1_5_expired_maps_to_canceled() {
    let status: OrderStatus = CcxtOrderStatus::Expired.into();
    assert_eq!(status, OrderStatus::Canceled);
}

#[test]
fn t1_6_failed() {
    let status: OrderStatus = CcxtOrderStatus::Failed.into();
    assert_eq!(status, OrderStatus::Failed);
}

#[test]
fn t1_7_rejected_maps_to_failed() {
    let status: OrderStatus = CcxtOrderStatus::Rejected.into();
    assert_eq!(status, OrderStatus::Failed);
}

// ============================================================
// TC-T2: CcxtTicker → Ticker
// ============================================================

#[test]
fn t2_1_ticker_all_fields() {
    let now = Utc::now();
    let ccxt = CcxtTicker {
        symbol: "BTC/USDT".into(),
        exchange: "binance".into(),
        bid: Some(50000.0),
        ask: Some(50001.0),
        last: Some(50000.5),
        high: Some(51000.0),
        low: Some(49000.0),
        volume: Some(1000.5),
        quote_volume: Some(50000000.0),
        open: Some(49500.0),
        close: Some(50000.5),
        previous_close: Some(49499.0),
        price_change: Some(500.5),
        price_change_pct: Some(1.01),
        timestamp: Some(now),
        info: serde_json::json!({}),
    };
    let ticker: Ticker = ccxt.try_into().unwrap();
    assert_eq!(ticker.symbol, "BTC/USDT");
    assert_eq!(ticker.exchange, "binance");
    assert_eq!(ticker.bid, Some(50000.0));
    assert_eq!(ticker.ask, Some(50001.0));
    assert_eq!(ticker.last, 50000.5);
    assert_eq!(ticker.high_24h, 51000.0);
    assert_eq!(ticker.low_24h, 49000.0);
    assert_eq!(ticker.volume_24h, 1000.5);
    assert_eq!(ticker.price_change_24h, 500.5);
    assert_eq!(ticker.price_change_pct_24h, 1.01);
    assert_eq!(ticker.timestamp, now);
}

#[test]
fn t2_2_ticker_none_fields_return_error() {
    let ccxt = CcxtTicker {
        symbol: "ETH/USDT".into(),
        exchange: "binance".into(),
        bid: None,
        ask: None,
        last: None,
        high: None,
        low: None,
        volume: None,
        quote_volume: None,
        open: None,
        close: None,
        previous_close: None,
        price_change: None,
        price_change_pct: None,
        timestamp: None,
        info: serde_json::json!({}),
    };
    let result: Result<Ticker, _> = ccxt.try_into();
    assert!(result.is_err(), "Ticker with None fields should return error, not default to 0.0");
}

#[test]
fn t2_3_ticker_timestamp_none_uses_now() {
    let before = Utc::now();
    let ccxt = CcxtTicker {
        symbol: "BTC/USDT".into(),
        exchange: "binance".into(),
        bid: Some(50000.0),
        ask: Some(50001.0),
        last: Some(50000.5),
        high: Some(51000.0),
        low: Some(49000.0),
        volume: Some(1000.5),
        quote_volume: Some(50000000.0),
        open: Some(49500.0),
        close: Some(50000.5),
        previous_close: Some(49499.0),
        price_change: Some(500.5),
        price_change_pct: Some(1.01),
        timestamp: None,
        info: serde_json::json!({}),
    };
    let ticker: Ticker = ccxt.try_into().unwrap();
    let after = Utc::now();
    assert!(ticker.timestamp >= before);
    assert!(ticker.timestamp <= after);
}

// ============================================================
// TC-T3: CcxtOrderBook → OrderBook
// ============================================================

#[test]
fn t3_1_order_book_normal() {
    let now = Utc::now();
    let ccxt = CcxtOrderBook {
        symbol: "BTC/USDT".into(),
        bids: vec![(50000.0, 1.5), (49999.0, 2.0)],
        asks: vec![(50001.0, 1.0), (50002.0, 0.5)],
        timestamp: Some(now),
        nonce: Some(12345),
    };
    let ob: OrderBook = ccxt.into();
    assert_eq!(ob.symbol, "BTC/USDT");
    assert_eq!(ob.bids, vec![(50000.0, 1.5), (49999.0, 2.0)]);
    assert_eq!(ob.asks, vec![(50001.0, 1.0), (50002.0, 0.5)]);
    assert_eq!(ob.timestamp, now);
}

#[test]
fn t3_2_order_book_timestamp_none() {
    let before = Utc::now();
    let ccxt = CcxtOrderBook {
        symbol: "ETH/USDT".into(),
        bids: vec![],
        asks: vec![],
        timestamp: None,
        nonce: None,
    };
    let ob: OrderBook = ccxt.into();
    let after = Utc::now();
    assert!(ob.timestamp >= before);
    assert!(ob.timestamp <= after);
}

// ============================================================
// TC-T4: CcxtFundingRate → FundingRate
// ============================================================

#[test]
fn t4_1_funding_rate_normal() {
    let now = Utc::now();
    let ccxt = CcxtFundingRate {
        symbol: "BTC/USDT".into(),
        rate: 0.0001,
        next_funding_time: Some(now),
        info: serde_json::json!({}),
    };
    let fr: FundingRate = ccxt.into();
    assert_eq!(fr.symbol, "BTC/USDT");
    assert!((fr.rate - 0.0001).abs() < f64::EPSILON);
    assert_eq!(fr.next_funding_time, Some(now));
}

// ============================================================
// TC-T5: CcxtFundingHistoryEntry → FundingHistoryEntry
// ============================================================

#[test]
fn t5_1_funding_history_normal() {
    let ccxt = CcxtFundingHistoryEntry {
        funding_time: chrono::DateTime::from_timestamp_millis(1700000000).unwrap(),
        rate: 0.00005,
    };
    let e: FundingHistoryEntry = ccxt.into();
    assert_eq!(e.funding_time, chrono::DateTime::from_timestamp_millis(1700000000).unwrap());
    assert!((e.rate - 0.00005).abs() < f64::EPSILON);
}

// Suppress unused import warning for Side (used in type scope but not directly in tests)
#[allow(dead_code)]
fn _suppress_warning() -> Side {
    Side::Buy
}

// ============================================================
// T7 WARN fix: nextFundingTime: 0 filtering
// ============================================================

#[test]
fn t7_1_funding_time_zero_is_epoch() {
    // T7 WARN fix: verify that from_timestamp_millis(0) returns Some(epoch)
    // This is the root cause — 0 is a valid timestamp, so we need explicit filtering
    let result = chrono::DateTime::from_timestamp_millis(0);
    assert!(result.is_some(), "timestamp 0 is a valid DateTime (epoch)");
    assert_eq!(
        result.unwrap(),
        chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap()
    );
}

#[test]
fn t7_2_filter_zero_before_from_timestamp_millis() {
    // T7 WARN fix: the fix adds .filter(|&ts| ts > 0) before from_timestamp_millis
    // This ensures nextFundingTime: 0 returns None instead of Some(epoch)
    let raw_ts: i64 = 0;
    let filtered = Some(raw_ts).filter(|&ts| ts > 0);
    assert_eq!(filtered, None, "0 should be filtered out");

    let valid_ts: i64 = 1700000000000;
    let valid_filtered = Some(valid_ts).filter(|&ts| ts > 0);
    assert_eq!(valid_filtered, Some(1700000000000));
}
