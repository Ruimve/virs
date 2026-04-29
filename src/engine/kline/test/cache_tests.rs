use crate::engine::kline::*;
use crate::engine::kline::cache::SymbolCache;

fn make_candle(open_time: i64, closed: bool) -> Candle {
    Candle {
        open_time,
        close_time: open_time + 59_999,
        open: 100.0,
        high: 110.0,
        low: 90.0,
        close: 105.0,
        volume: 50.0,
        quote_volume: 5000.0,
        trades: 100,
        closed,
    }
}

fn make_candle_with_price(open_time: i64, price: f64, closed: bool) -> Candle {
    Candle {
        open_time,
        close_time: open_time + 59_999,
        open: price,
        high: price + 10.0,
        low: price - 10.0,
        close: price + 5.0,
        volume: 50.0,
        quote_volume: 5000.0,
        trades: 100,
        closed,
    }
}

#[test]
fn test_symbol_cache_new() {
    let cache = SymbolCache::new();
    for tf in Timeframe::all() {
        assert_eq!(cache.candle_count(*tf), 0);
    }
    assert!(cache.is_empty());
}

#[test]
fn test_update_candle_append() {
    let mut cache = SymbolCache::new();
    let c1 = make_candle(0, true);
    let c2 = make_candle(60_000, true);
    cache.update_candle(Timeframe::M1, c1);
    cache.update_candle(Timeframe::M1, c2);
    assert_eq!(cache.candle_count(Timeframe::M1), 2);
}

#[test]
fn test_update_candle_same_open_time_overwrite() {
    let mut cache = SymbolCache::new();
    let c1 = make_candle_with_price(0, 100.0, false);
    let c2 = make_candle_with_price(0, 110.0, true);
    cache.update_candle(Timeframe::M1, c1);
    cache.update_candle(Timeframe::M1, c2);
    assert_eq!(cache.candle_count(Timeframe::M1), 1);
    let klines = cache.get_klines(Timeframe::M1);
    assert_eq!(klines[0].open, 110.0);
    assert!(klines[0].closed);
}

#[test]
fn test_update_candle_older_update_in_place() {
    let mut cache = SymbolCache::new();
    let c1 = make_candle_with_price(0, 100.0, false);
    let c2 = make_candle_with_price(60_000, 110.0, false);
    let c1_update = make_candle_with_price(0, 105.0, true);
    cache.update_candle(Timeframe::M1, c1);
    cache.update_candle(Timeframe::M1, c2);
    cache.update_candle(Timeframe::M1, c1_update);
    assert_eq!(cache.candle_count(Timeframe::M1), 2);
    let klines = cache.get_klines(Timeframe::M1);
    assert_eq!(klines[0].open, 105.0);
    assert!(klines[0].closed);
}

#[test]
fn test_close_candle() {
    let mut cache = SymbolCache::new();
    let c = make_candle(0, false);
    cache.update_candle(Timeframe::M1, c);
    assert!(!cache.get_klines(Timeframe::M1)[0].closed);
    cache.close_candle(Timeframe::M1, 0);
    assert!(cache.get_klines(Timeframe::M1)[0].closed);
}

#[test]
fn test_close_candle_nonexistent() {
    let mut cache = SymbolCache::new();
    cache.close_candle(Timeframe::M1, 999);
    assert_eq!(cache.candle_count(Timeframe::M1), 0);
}

#[test]
fn test_last_closed_1m() {
    let mut cache = SymbolCache::new();
    assert!(cache.last_closed_1m().is_none());

    cache.update_candle(Timeframe::M1, make_candle(0, true));
    cache.update_candle(Timeframe::M1, make_candle(60_000, false));
    let last_closed = cache.last_closed_1m().unwrap();
    assert_eq!(last_closed.open_time, 0);
    assert!(last_closed.closed);
}

#[test]
fn test_last_1m() {
    let mut cache = SymbolCache::new();
    assert!(cache.last_1m().is_none());

    cache.update_candle(Timeframe::M1, make_candle(0, true));
    cache.update_candle(Timeframe::M1, make_candle(60_000, false));
    let last = cache.last_1m().unwrap();
    assert_eq!(last.open_time, 60_000);
}

#[test]
fn test_replace_timeframe() {
    let mut cache = SymbolCache::new();
    cache.update_candle(Timeframe::M1, make_candle(0, true));
    let new_candles: Vec<Candle> = (0..10)
        .map(|i| make_candle(i as i64 * 60_000, true))
        .collect();
    cache.replace_timeframe(Timeframe::M1, new_candles);
    assert_eq!(cache.candle_count(Timeframe::M1), 10);
}

#[test]
fn test_replace_timeframe_truncation() {
    let mut cache = SymbolCache::new();
    let limit = Timeframe::M1.default_limit();
    let new_candles: Vec<Candle> = (0..limit + 100)
        .map(|i| make_candle(i as i64 * 60_000, true))
        .collect();
    cache.replace_timeframe(Timeframe::M1, new_candles);
    assert_eq!(cache.candle_count(Timeframe::M1), limit);
    let klines = cache.get_klines(Timeframe::M1);
    assert_eq!(klines[0].open_time, 100 * 60_000);

    // Backfill truncation assertion (merged from test_backfill_truncation)
    let mut cache2 = SymbolCache::new();
    let limit2 = Timeframe::M1.default_limit();
    let candles: Vec<Candle> = (0..limit2 + 100)
        .map(|i| make_candle(i as i64 * 60_000, true))
        .collect();
    cache2.backfill_timeframe(Timeframe::M1, candles);
    assert_eq!(cache2.candle_count(Timeframe::M1), limit2);
}

#[test]
fn test_backfill_timeframe_empty_cache() {
    let mut cache = SymbolCache::new();
    let candles: Vec<Candle> = (0..5)
        .map(|i| make_candle(i as i64 * 60_000, true))
        .collect();
    cache.backfill_timeframe(Timeframe::M1, candles);
    assert_eq!(cache.candle_count(Timeframe::M1), 5);
}

#[test]
fn test_backfill_timeframe_append_new() {
    let mut cache = SymbolCache::new();
    cache.update_candle(Timeframe::M1, make_candle(0, true));
    cache.update_candle(Timeframe::M1, make_candle(60_000, true));
    let backfill: Vec<Candle> = (2..5)
        .map(|i| make_candle(i as i64 * 60_000, true))
        .collect();
    cache.backfill_timeframe(Timeframe::M1, backfill);
    assert_eq!(cache.candle_count(Timeframe::M1), 5);
}

#[test]
fn test_backfill_overwrite_unclosed_with_closed() {
    let mut cache = SymbolCache::new();
    cache.update_candle(Timeframe::M1, make_candle_with_price(0, 100.0, false));
    let backfill = vec![make_candle_with_price(0, 105.0, true)];
    cache.backfill_timeframe(Timeframe::M1, backfill);
    let klines = cache.get_klines(Timeframe::M1);
    assert_eq!(klines[0].open, 105.0);
    assert!(klines[0].closed);
}

#[test]
fn test_backfill_no_overwrite_closed_with_closed() {
    let mut cache = SymbolCache::new();
    cache.update_candle(Timeframe::M1, make_candle_with_price(0, 100.0, true));
    let backfill = vec![make_candle_with_price(0, 105.0, true)];
    cache.backfill_timeframe(Timeframe::M1, backfill);
    let klines = cache.get_klines(Timeframe::M1);
    assert_eq!(klines[0].open, 100.0);
}

#[test]
fn test_backfill_no_overwrite_closed_with_unclosed() {
    let mut cache = SymbolCache::new();
    cache.update_candle(Timeframe::M1, make_candle_with_price(0, 100.0, true));
    let backfill = vec![make_candle_with_price(0, 105.0, false)];
    cache.backfill_timeframe(Timeframe::M1, backfill);
    let klines = cache.get_klines(Timeframe::M1);
    assert_eq!(klines[0].open, 100.0);
    assert!(klines[0].closed);
}

#[test]
fn test_capacity_enforcement_on_update() {
    let mut cache = SymbolCache::new();
    let limit = Timeframe::M1.default_limit();
    for i in 0..limit + 50 {
        cache.update_candle(Timeframe::M1, make_candle(i as i64 * 60_000, true));
    }
    assert_eq!(cache.candle_count(Timeframe::M1), limit);
}

#[test]
fn test_get_all_timeframes() {
    let mut cache = SymbolCache::new();
    cache.update_candle(Timeframe::M1, make_candle(0, true));
    cache.update_candle(Timeframe::M5, make_candle(0, true));
    let all = cache.get_all_timeframes();
    assert_eq!(all.m1.len(), 1);
    assert_eq!(all.m5.len(), 1);
    assert_eq!(all.m15.len(), 0);
}

#[test]
fn test_get_klines_nonexistent_timeframe() {
    let cache = SymbolCache::new();
    let klines = cache.get_klines(Timeframe::M1);
    assert!(klines.is_empty());
}

#[test]
fn test_is_empty_after_data() {
    let mut cache = SymbolCache::new();
    cache.update_candle(Timeframe::M1, make_candle(0, true));
    assert!(!cache.is_empty());
}

#[test]
fn test_multiple_timeframes_independent() {
    let mut cache = SymbolCache::new();
    cache.update_candle(Timeframe::M1, make_candle(0, true));
    cache.update_candle(Timeframe::M1, make_candle(60_000, true));
    cache.update_candle(Timeframe::M5, make_candle(0, true));
    assert_eq!(cache.candle_count(Timeframe::M1), 2);
    assert_eq!(cache.candle_count(Timeframe::M5), 1);
    assert_eq!(cache.candle_count(Timeframe::H1), 0);
}


#[test]
fn test_timeframe_buffer_push_or_update_order_preserved() {
    let mut cache = SymbolCache::new();
    cache.update_candle(Timeframe::M1, make_candle_with_price(0, 100.0, true));
    cache.update_candle(Timeframe::M1, make_candle_with_price(60_000, 110.0, true));
    cache.update_candle(Timeframe::M1, make_candle_with_price(120_000, 120.0, true));
    let klines = cache.get_klines(Timeframe::M1);
    assert_eq!(klines.len(), 3);
    assert_eq!(klines[0].open_time, 0);
    assert_eq!(klines[1].open_time, 60_000);
    assert_eq!(klines[2].open_time, 120_000);
}
