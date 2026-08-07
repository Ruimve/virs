use crate::aggregator::{candle_from_1m, Aggregator};
use crate::types::align_open_time;
use virs_type::{Candle, Timeframe};

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

#[test]
fn a2_1_is_last_1m_in_group_m5() {
    let group_start = align_open_time(BASE, Timeframe::M5);
    let fifth = make_1m(group_start + 4 * 60_000, 100.0, 100.0, 100.0, 100.0, true);
    assert!(Aggregator::is_last_1m_in_group(&fifth, Timeframe::M5));
}

#[test]
fn a2_2_is_last_1m_not_last() {
    let group_start = align_open_time(BASE, Timeframe::M5);
    let third = make_1m(group_start + 2 * 60_000, 100.0, 100.0, 100.0, 100.0, true);
    assert!(!Aggregator::is_last_1m_in_group(&third, Timeframe::M5));
}

#[test]
fn a2_3_is_last_1m_in_group_h1() {
    let group_start = align_open_time(BASE, Timeframe::H1);
    let sixtieth = make_1m(group_start + 59 * 60_000, 100.0, 100.0, 100.0, 100.0, true);
    assert!(Aggregator::is_last_1m_in_group(&sixtieth, Timeframe::H1));
}

#[test]
fn a2_4_is_last_1m_exact_boundary() {
    let group_start = align_open_time(BASE, Timeframe::M5);
    let first = make_1m(group_start, 100.0, 100.0, 100.0, 100.0, true);
    assert!(!Aggregator::is_last_1m_in_group(&first, Timeframe::M5));
}

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
            make_1m(
                start + i * 60_000,
                price,
                price + 2.0,
                price - 2.0,
                price + 1.0,
                true,
            )
        })
        .collect();

    let result = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::M5);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].open, 100.0);
    assert_eq!(result[0].close, 105.0);
    assert_eq!(result[0].high, 106.0);
    assert_eq!(result[0].low, 98.0);
    assert!(result[0].closed);
    assert_eq!(result[0].volume, 500.0);
}

#[test]
fn a3_4_aggregate_m5_partial() {
    let start = align_open_time(BASE, Timeframe::M5);
    let candles: Vec<Candle> = (0..3)
        .map(|i| {
            let price = 100.0 + i as f64;
            make_1m(
                start + i * 60_000,
                price,
                price + 1.0,
                price - 1.0,
                price,
                true,
            )
        })
        .collect();

    let result = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::M5);
    assert_eq!(result.len(), 1);
    assert!(!result[0].closed);
}

#[test]
fn a3_5_aggregate_multi_group() {
    let start = align_open_time(BASE, Timeframe::M5);

    let candles: Vec<Candle> = (0..7)
        .map(|i| {
            let price = 100.0 + i as f64;
            make_1m(
                start + i * 60_000,
                price,
                price + 1.0,
                price - 1.0,
                price,
                true,
            )
        })
        .collect();

    let result = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::M5);
    assert_eq!(result.len(), 2);
    assert!(result[0].closed);
    assert!(!result[1].closed);
}

#[test]
fn a4_1_align_then_aggregate() {
    let start = align_open_time(BASE, Timeframe::M5);
    let candles: Vec<Candle> = (0..5)
        .map(|i| make_1m(start + i * 60_000, 100.0, 101.0, 99.0, 100.0, true))
        .collect();
    let result = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::M5);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].open_time, start);
    assert_eq!(result[0].close_time, start + Timeframe::M5.ms() - 1);
}

#[test]
fn a4_2_candle_from_1m_then_aggregate() {
    let start = align_open_time(BASE, Timeframe::M5);
    let c1 = make_1m(start, 100.0, 102.0, 98.0, 101.0, true);
    let from_1m = candle_from_1m(&c1, Timeframe::M5);
    let aggregated = Aggregator::aggregate_1m_to_timeframe(&[c1], Timeframe::M5);
    assert_eq!(aggregated.len(), 1);
    assert_eq!(from_1m.open_time, aggregated[0].open_time);
    assert_eq!(from_1m.open, aggregated[0].open);
    assert_eq!(from_1m.close, aggregated[0].close);
}

#[test]
fn a4_3_aggregate_then_cache_update() {
    use crate::cache::SymbolCache;

    let start = align_open_time(BASE, Timeframe::M5);
    let candles: Vec<Candle> = (0..5)
        .map(|i| {
            make_1m(
                start + i * 60_000,
                100.0 + i as f64,
                101.0,
                99.0,
                100.0 + i as f64,
                true,
            )
        })
        .collect();
    let aggregated = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::M5);

    let mut cache = SymbolCache::new();
    for c in &aggregated {
        cache.update_candle(Timeframe::M5, c.clone());
    }
    let klines = cache.get_klines(Timeframe::M5);
    assert_eq!(klines.len(), 1);
    assert_eq!(klines[0].open_time, start);
}

#[test]
fn a4_4_aggregate_with_gap() {
    let start = align_open_time(BASE, Timeframe::M5);
    let c1 = make_1m(start, 100.0, 101.0, 99.0, 100.5, true);
    let c3 = make_1m(start + 2 * 60_000, 100.5, 102.0, 100.0, 101.0, true);

    let candles = vec![c1, c3];
    let aggregated = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::M5);

    assert_eq!(aggregated.len(), 1);
    assert_eq!(aggregated[0].volume, 200.0);
}

#[test]
fn a4_5_aggregate_full_day_to_d1() {
    let start = align_open_time(BASE, Timeframe::D1);
    let candles: Vec<Candle> = (0..1440)
        .map(|i| {
            let price = 100.0 + (i as f64) * 0.01;
            make_1m(
                start + i * 60_000,
                price,
                price + 0.5,
                price - 0.5,
                price + 0.01,
                true,
            )
        })
        .collect();

    let result = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::D1);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].open, 100.0);
    assert!((result[0].close - (100.0 + 1439.0 * 0.01 + 0.01)).abs() < 1e-10);
    assert_eq!(result[0].volume, 1440.0 * 100.0);
    assert!(result[0].closed);
}
