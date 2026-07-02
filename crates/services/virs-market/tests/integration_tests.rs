//! Integration tests for virs-market — cross-module chain verification.

use virs_market::aggregator::{candle_from_1m, Aggregator};
use virs_market::cache::SymbolCache;
use virs_market::source::timeframe_str_to_ms;
use virs_market::{
    align_open_time, subscription_key, Candle, Timeframe,
};

// Use a base time that is divisible by 60_000
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

// ── INT-1: Timeframe alignment ─────────────────────────────

#[test]
fn int_1_2_align_then_aggregate() {
    // align_open_time → aggregate should produce candles with aligned open_time
    let start = align_open_time(BASE, Timeframe::M5);
    let candles: Vec<Candle> = (0..5)
        .map(|i| make_1m(start + i * 60_000, 100.0, 101.0, 99.0, 100.0, true))
        .collect();
    let result = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::M5);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].open_time, start); // aligned
    assert_eq!(result[0].close_time, start + Timeframe::M5.ms() - 1);
}

// ── INT-2: Aggregator → Cache chain ────────────────────────

#[test]
fn int_2_1_candle_from_1m_then_aggregate() {
    // candle_from_1m and aggregate_1m_to_timeframe should produce consistent results
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
fn int_2_2_aggregate_then_cache_update() {
    let start = align_open_time(BASE, Timeframe::M5);
    let candles: Vec<Candle> = (0..5)
        .map(|i| make_1m(start + i * 60_000, 100.0 + i as f64, 101.0, 99.0, 100.0 + i as f64, true))
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
fn int_2_3_cache_get_all_timeframes() {
    let mut cache = SymbolCache::new();
    cache.update_candle(Timeframe::M1, make_1m(BASE, 100.0, 101.0, 99.0, 100.0, true));
    cache.update_candle(Timeframe::M5, make_1m(BASE, 100.0, 101.0, 99.0, 100.0, true));
    cache.update_candle(Timeframe::H1, make_1m(BASE, 100.0, 101.0, 99.0, 100.0, true));

    let all = cache.get_all_timeframes();
    assert_eq!(all.m1.len(), 1);
    assert_eq!(all.m5.len(), 1);
    assert_eq!(all.h1.len(), 1);
    assert!(all.m15.is_empty());
    assert!(all.h4.is_empty());
    assert!(all.d1.is_empty());
}

// ── INT-3: subscription_key + align ────────────────────────

#[test]
fn int_3_1_subscription_key_then_check() {
    let key1 = subscription_key("binance", "BTC/USDT");
    let key2 = subscription_key("binance", "BTC/USDT");
    assert_eq!(key1, key2);
    assert!(key1.contains(':'));
    assert!(key1.starts_with("binance:"));
}

#[test]
fn int_3_2_align_multi_timeframe() {
    // Same open_time aligned to different timeframes should produce different boundaries
    let time = BASE + 123_456; // not aligned
    let m1 = align_open_time(time, Timeframe::M1);
    let m5 = align_open_time(time, Timeframe::M5);
    let h1 = align_open_time(time, Timeframe::H1);
    let d1 = align_open_time(time, Timeframe::D1);

    assert!(m5 <= m1); // M5 truncates more than M1
    assert!(h1 <= m5); // H1 truncates more than M5
    assert!(d1 <= h1);
    assert_eq!(m1 % 60_000, 0);
    assert_eq!(m5 % 300_000, 0);
    assert_eq!(h1 % 3_600_000, 0);
    assert_eq!(d1 % 86_400_000, 0);
}

// ── INT-5: Gap detection + full day aggregation ────────────

#[test]
fn int_5_1_gap_detection_logic() {
    // Simulate a gap: 2 candles with a 2-minute gap between them
    let start = align_open_time(BASE, Timeframe::M5); // align to M5 group boundary
    let c1 = make_1m(start, 100.0, 101.0, 99.0, 100.5, true);
    let c3 = make_1m(start + 2 * 60_000, 100.5, 102.0, 100.0, 101.0, true);
    // Gap: candle at start + 1*60_000 is missing

    let candles = vec![c1, c3];
    let aggregated = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::M5);
    // With gap, still produces a candle but volume reflects only 2 candles
    assert_eq!(aggregated.len(), 1);
    assert_eq!(aggregated[0].volume, 200.0); // 2 * 100.0
}

#[test]
fn int_5_2_aggregate_full_day_to_d1() {
    // 1440 1m candles → 1 D1 candle
    let start = align_open_time(BASE, Timeframe::D1);
    let candles: Vec<Candle> = (0..1440)
        .map(|i| {
            let price = 100.0 + (i as f64) * 0.01;
            make_1m(start + i * 60_000, price, price + 0.5, price - 0.5, price + 0.01, true)
        })
        .collect();

    let result = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::D1);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].open, 100.0); // first candle open
    assert!((result[0].close - (100.0 + 1439.0 * 0.01 + 0.01)).abs() < 1e-10);
    assert_eq!(result[0].volume, 1440.0 * 100.0); // 1440 * 100
    assert!(result[0].closed);
}

// ── INT-6: timeframe_str_to_ms ─────────────────────────────

#[test]
fn int_6_1_timeframe_str_to_ms() {
    assert_eq!(timeframe_str_to_ms("1m"), 60_000);
    assert_eq!(timeframe_str_to_ms("5m"), 300_000);
    assert_eq!(timeframe_str_to_ms("15m"), 900_000);
    assert_eq!(timeframe_str_to_ms("1h"), 3_600_000);
    assert_eq!(timeframe_str_to_ms("4h"), 14_400_000);
    assert_eq!(timeframe_str_to_ms("1d"), 86_400_000);
    // Unknown → defaults to 1m (60000)
    assert_eq!(timeframe_str_to_ms("invalid"), 60_000);
}
