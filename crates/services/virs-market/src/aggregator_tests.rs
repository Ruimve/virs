//! Unit tests for aggregator.rs

use crate::aggregator::{candle_from_1m, Aggregator};
use crate::types::{align_open_time, Candle, Timeframe};

// Use a base time that is divisible by 60_000 (1 minute)
const BASE: i64 = 1_700_000_040_000;

fn make_1m(open_time: i64, open: f64, high: f64, low: f64, close: f64, closed: bool) -> Candle {
    Candle {
        open_time,
        close_time: open_time + 59_999,
        open,
        high,
        low,
        close,
        volume: 100.0,
        quote_volume: 100.0 * close,
        trades: 10,
        closed,
    }
}

// ── candle_from_1m ─────────────────────────────────────────

#[test]
fn a1_1_candle_from_1m_basic() {
    let base = make_1m(BASE, 100.0, 105.0, 95.0, 102.0, true);
    let result = candle_from_1m(&base, Timeframe::M5);
    assert_eq!(result.open, 100.0);
    assert_eq!(result.high, 105.0);
    assert_eq!(result.low, 95.0);
    assert_eq!(result.close, 102.0);
    assert_eq!(result.volume, 100.0);
    assert_eq!(result.trades, 10);
}

#[test]
fn a1_2_candle_from_1m_align() {
    // BASE is already aligned to minute, align to M5
    let base = make_1m(BASE, 100.0, 105.0, 95.0, 102.0, true);
    let result = candle_from_1m(&base, Timeframe::M5);
    let expected_open = align_open_time(BASE, Timeframe::M5);
    assert_eq!(result.open_time, expected_open);
    assert_eq!(result.close_time, expected_open + Timeframe::M5.ms() - 1);
}

#[test]
fn a1_3_candle_from_1m_closed_false() {
    let base = make_1m(BASE, 100.0, 105.0, 95.0, 102.0, true);
    let result = candle_from_1m(&base, Timeframe::M5);
    assert!(!result.closed);
}

// ── is_last_1m_in_group ────────────────────────────────────

#[test]
fn a2_1_is_last_1m_in_group_m5() {
    // Use M5-aligned start, 5th minute → last in group
    let group_start = align_open_time(BASE, Timeframe::M5);
    let fifth = make_1m(group_start + 4 * 60_000, 100.0, 100.0, 100.0, 100.0, true);
    assert!(Aggregator::is_last_1m_in_group(&fifth, Timeframe::M5));
}

#[test]
fn a2_2_is_last_1m_not_last() {
    // 3rd minute of M5 group → not last
    let group_start = align_open_time(BASE, Timeframe::M5);
    let third = make_1m(group_start + 2 * 60_000, 100.0, 100.0, 100.0, 100.0, true);
    assert!(!Aggregator::is_last_1m_in_group(&third, Timeframe::M5));
}

#[test]
fn a2_3_is_last_1m_in_group_h1() {
    // 60th minute of H1 group → last
    let group_start = align_open_time(BASE, Timeframe::H1);
    let sixtieth = make_1m(group_start + 59 * 60_000, 100.0, 100.0, 100.0, 100.0, true);
    assert!(Aggregator::is_last_1m_in_group(&sixtieth, Timeframe::H1));
}

#[test]
fn a2_4_is_last_1m_exact_boundary() {
    // Candle exactly at group boundary start → not last for M5
    let group_start = align_open_time(BASE, Timeframe::M5);
    let first = make_1m(group_start, 100.0, 100.0, 100.0, 100.0, true);
    assert!(!Aggregator::is_last_1m_in_group(&first, Timeframe::M5));
}

// ── aggregate_1m_to_timeframe ──────────────────────────────

#[test]
fn a3_1_aggregate_empty() {
    let result = Aggregator::aggregate_1m_to_timeframe(&[], Timeframe::M5);
    assert!(result.is_empty());
}

#[test]
fn a3_2_aggregate_single_candle() {
    let c = make_1m(BASE, 100.0, 105.0, 95.0, 102.0, true);
    let result = Aggregator::aggregate_1m_to_timeframe(&[c], Timeframe::M5);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].open, 100.0);
    assert_eq!(result[0].close, 102.0);
}

#[test]
fn a3_3_aggregate_m5_full() {
    let start = align_open_time(BASE, Timeframe::M5);
    let candles: Vec<Candle> = (0..5)
        .map(|i| {
            let price = 100.0 + i as f64;
            make_1m(start + i * 60_000, price, price + 2.0, price - 2.0, price + 1.0, true)
        })
        .collect();

    let result = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::M5);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].open, 100.0); // first candle open
    assert_eq!(result[0].close, 105.0); // last candle close = 104+1
    assert_eq!(result[0].high, 106.0); // max high = 104+2
    assert_eq!(result[0].low, 98.0); // min low = 100-2
    assert!(result[0].closed); // last 1m is closed → M5 closed
    assert_eq!(result[0].volume, 500.0); // 5 * 100
}

#[test]
fn a3_4_aggregate_m5_partial() {
    let start = align_open_time(BASE, Timeframe::M5);
    let candles: Vec<Candle> = (0..3)
        .map(|i| {
            let price = 100.0 + i as f64;
            make_1m(start + i * 60_000, price, price + 1.0, price - 1.0, price, true)
        })
        .collect();

    let result = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::M5);
    assert_eq!(result.len(), 1);
    assert!(!result[0].closed); // only 3 of 5 → not closed
}

#[test]
fn a3_5_aggregate_multi_group() {
    let start = align_open_time(BASE, Timeframe::M5);
    // 7 candles → 2 groups (5 + 2)
    let candles: Vec<Candle> = (0..7)
        .map(|i| {
            let price = 100.0 + i as f64;
            make_1m(start + i * 60_000, price, price + 1.0, price - 1.0, price, true)
        })
        .collect();

    let result = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::M5);
    assert_eq!(result.len(), 2);
    assert!(result[0].closed); // first group complete
    assert!(!result[1].closed); // second group incomplete (2/5)
}
