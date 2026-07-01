//! Unit tests for cache.rs

use crate::cache::SymbolCache;
use crate::types::{Candle, Timeframe};

fn make_candle(open_time: i64, close: f64, closed: bool) -> Candle {
    Candle {
        open_time,
        close_time: open_time + 59_999,
        open: close,
        high: close + 1.0,
        low: close - 1.0,
        close,
        volume: 100.0,
        quote_volume: 100.0 * close,
        trades: 10,
        closed,
    }
}

// ── update_candle + get_klines ─────────────────────────────

#[test]
fn c1_1_update_and_get() {
    let mut cache = SymbolCache::new();
    let candle = make_candle(1_700_000_000_000, 100.0, false);
    cache.update_candle(Timeframe::M1, candle.clone());
    let klines = cache.get_klines(Timeframe::M1);
    assert_eq!(klines.len(), 1);
    assert_eq!(klines[0].open_time, candle.open_time);
    assert_eq!(klines[0].close, 100.0);
}

#[test]
fn c1_2_update_same_open_time() {
    let mut cache = SymbolCache::new();
    let c1 = make_candle(1_700_000_000_000, 100.0, false);
    let c2 = make_candle(1_700_000_000_000, 105.0, false);
    cache.update_candle(Timeframe::M1, c1);
    cache.update_candle(Timeframe::M1, c2);
    let klines = cache.get_klines(Timeframe::M1);
    assert_eq!(klines.len(), 1);
    assert_eq!(klines[0].close, 105.0); // overwritten
}

#[test]
fn c1_3_update_old_candle() {
    let mut cache = SymbolCache::new();
    let c1 = make_candle(1_700_000_060_000, 100.0, true);
    let c2 = make_candle(1_700_000_000_000, 95.0, true); // older
    cache.update_candle(Timeframe::M1, c1);
    cache.update_candle(Timeframe::M1, c2);
    let klines = cache.get_klines(Timeframe::M1);
    assert_eq!(klines.len(), 2);
    // Old candle should be found (inserted in place)
    assert!(klines.iter().any(|c| c.open_time == 1_700_000_000_000));
}

#[test]
fn c1_4_max_size_eviction() {
    let mut cache = SymbolCache::new();
    // M1 default_limit = 1000
    for i in 0..1005 {
        cache.update_candle(Timeframe::M1, make_candle(i * 60_000, 100.0, true));
    }
    let klines = cache.get_klines(Timeframe::M1);
    assert_eq!(klines.len(), 1000); // evicted oldest 5
    // First candle should be at index 5
    assert_eq!(klines[0].open_time, 5 * 60_000);
}

// ── close_candle + last_closed ─────────────────────────────

#[test]
fn c2_1_close_candle() {
    let mut cache = SymbolCache::new();
    cache.update_candle(Timeframe::M1, make_candle(1_700_000_000_000, 100.0, false));
    cache.close_candle(Timeframe::M1, 1_700_000_000_000);
    let klines = cache.get_klines(Timeframe::M1);
    assert!(klines[0].closed);
}

#[test]
fn c2_2_last_closed_1m() {
    let mut cache = SymbolCache::new();
    cache.update_candle(Timeframe::M1, make_candle(1_700_000_000_000, 100.0, true));
    cache.update_candle(Timeframe::M1, make_candle(1_700_000_060_000, 101.0, false));
    let last_closed = cache.last_closed_1m().unwrap();
    assert_eq!(last_closed.open_time, 1_700_000_000_000);
    assert!(last_closed.closed);
}

// ── replace_timeframe ──────────────────────────────────────

#[test]
fn c3_1_replace_timeframe() {
    let mut cache = SymbolCache::new();
    cache.update_candle(Timeframe::M1, make_candle(1_700_000_000_000, 100.0, true));
    let new_candles: Vec<Candle> = (0..5)
        .map(|i| make_candle(i * 60_000, 100.0 + i as f64, true))
        .collect();
    cache.replace_timeframe(Timeframe::M1, new_candles);
    let klines = cache.get_klines(Timeframe::M1);
    assert_eq!(klines.len(), 5);
    assert_eq!(klines[0].open_time, 0);
    assert_eq!(klines[4].close, 104.0);
}
