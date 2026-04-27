use crate::kline::*;
use crate::kline::aggregator::Aggregator;
use crate::kline::cache::SymbolCache;

fn make_1m_candle(open_time: i64, o: f64, h: f64, l: f64, c: f64, v: f64, closed: bool) -> Candle {
    Candle {
        open_time,
        close_time: open_time + 59_999,
        open: o,
        high: h,
        low: l,
        close: c,
        volume: v,
        quote_volume: v * c,
        trades: 100,
        closed,
    }
}

fn make_1m_sequence(start_time: i64, count: usize, base_price: f64) -> Vec<Candle> {
    (0..count)
        .map(|i| {
            let t = start_time + i as i64 * 60_000;
            let price = base_price + i as f64;
            make_1m_candle(t, price, price + 5.0, price - 1.0, price + 1.0, 10.0, true)
        })
        .collect()
}

#[test]
fn test_aggregate_5m_single_group() {
    let candles = vec![
        make_1m_candle(0, 100.0, 105.0, 99.0, 102.0, 10.0, true),
        make_1m_candle(60_000, 102.0, 108.0, 101.0, 106.0, 12.0, true),
        make_1m_candle(120_000, 106.0, 110.0, 104.0, 107.0, 8.0, true),
        make_1m_candle(180_000, 107.0, 112.0, 106.0, 109.0, 15.0, true),
        make_1m_candle(240_000, 109.0, 115.0, 108.0, 111.0, 11.0, true),
    ];

    let result = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::M5);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].open_time, 0);
    assert_eq!(result[0].open, 100.0);
    assert_eq!(result[0].high, 115.0);
    assert_eq!(result[0].low, 99.0);
    assert_eq!(result[0].close, 111.0);
    assert!((result[0].volume - 56.0).abs() < 0.001);
    assert!(result[0].closed);

    // High/low tracking with extreme values (merged from test_aggregate_high_low_tracking)
    let candles_hl = vec![
        make_1m_candle(0, 100.0, 105.0, 98.0, 102.0, 10.0, true),
        make_1m_candle(60_000, 102.0, 120.0, 101.0, 106.0, 12.0, true),
        make_1m_candle(120_000, 106.0, 108.0, 90.0, 107.0, 8.0, true),
        make_1m_candle(180_000, 107.0, 112.0, 106.0, 109.0, 15.0, true),
        make_1m_candle(240_000, 109.0, 115.0, 108.0, 111.0, 11.0, true),
    ];
    let result_hl = Aggregator::aggregate_1m_to_timeframe(&candles_hl, Timeframe::M5);
    assert_eq!(result_hl[0].high, 120.0);
    assert_eq!(result_hl[0].low, 90.0);

    // Volume accumulation with distinct volumes (merged from test_aggregate_volume_accumulation)
    let candles_vol = vec![
        make_1m_candle(0, 100.0, 105.0, 99.0, 102.0, 10.0, true),
        make_1m_candle(60_000, 102.0, 108.0, 101.0, 106.0, 20.0, true),
        make_1m_candle(120_000, 106.0, 110.0, 104.0, 107.0, 30.0, true),
        make_1m_candle(180_000, 107.0, 112.0, 106.0, 109.0, 40.0, true),
        make_1m_candle(240_000, 109.0, 115.0, 108.0, 111.0, 50.0, true),
    ];
    let result_vol = Aggregator::aggregate_1m_to_timeframe(&candles_vol, Timeframe::M5);
    assert!((result_vol[0].volume - 150.0).abs() < 0.001);
    assert_eq!(result_vol[0].trades, 500);
}

#[test]
fn test_aggregate_two_5m_groups() {
    let candles = make_1m_sequence(0, 10, 100.0);
    let result = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::M5);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].open_time, 0);
    assert_eq!(result[1].open_time, 300_000);
}

#[test]
fn test_aggregate_empty() {
    let result = Aggregator::aggregate_1m_to_timeframe(&[], Timeframe::M5);
    assert!(result.is_empty());
}

#[test]
fn test_aggregate_single_candle() {
    let candles = vec![make_1m_candle(0, 100.0, 105.0, 99.0, 102.0, 10.0, true)];
    let result = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::M5);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].open, 100.0);
    assert_eq!(result[0].close, 102.0);
    assert!(!result[0].closed);
}

#[test]
fn test_aggregate_partial_group_unclosed() {
    let candles = make_1m_sequence(0, 3, 100.0);
    let result = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::M5);
    assert_eq!(result.len(), 1);
    assert!(!result[0].closed);
}

#[test]
fn test_aggregate_15m() {
    let candles = make_1m_sequence(0, 15, 100.0);
    let result = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::M15);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].open_time, 0);
    assert_eq!(result[0].close_time, 900_000 - 1);
    assert!(result[0].closed);
}

#[test]
fn test_aggregate_1h() {
    let candles = make_1m_sequence(0, 60, 100.0);
    let result = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::H1);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].open_time, 0);
    assert_eq!(result[0].close_time, 3_600_000 - 1);
    assert!(result[0].closed);
}

#[test]
fn test_aggregate_4h() {
    let candles = make_1m_sequence(0, 240, 100.0);
    let result = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::H4);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].open_time, 0);
    assert_eq!(result[0].close_time, 14_400_000 - 1);
    assert!(result[0].closed);
}

#[test]
fn test_aggregate_1d() {
    let candles = make_1m_sequence(0, 1440, 100.0);
    let result = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::D1);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].open_time, 0);
    assert_eq!(result[0].close_time, 86_400_000 - 1);
    assert!(result[0].closed);
}

#[test]
fn test_aggregate_1d_two_days() {
    let candles = make_1m_sequence(0, 2880, 100.0);
    let result = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::D1);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].open_time, 0);
    assert_eq!(result[1].open_time, 86_400_000);
}

#[test]
fn test_aggregate_mixed_closed_unclosed() {
    let mut candles = make_1m_sequence(0, 4, 100.0);
    if let Some(last) = candles.last_mut() {
        last.closed = false;
    }
    let result = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::M5);
    assert_eq!(result.len(), 1);
    assert!(!result[0].closed);
}


#[test]
fn test_is_last_1m_in_group() {
    let c1 = make_1m_candle(0, 100.0, 100.0, 100.0, 100.0, 1.0, true);
    let c2 = make_1m_candle(240_000, 100.0, 100.0, 100.0, 100.0, 1.0, true);
    let c3 = make_1m_candle(300_000, 100.0, 100.0, 100.0, 100.0, 1.0, true);

    assert!(!Aggregator::is_last_1m_in_group(&c1, Timeframe::M5));
    assert!(Aggregator::is_last_1m_in_group(&c2, Timeframe::M5));
    assert!(!Aggregator::is_last_1m_in_group(&c3, Timeframe::M5));

    // 1h assertions (merged from test_is_last_1m_in_group_1h)
    let c_mid = make_1m_candle(1_800_000, 100.0, 100.0, 100.0, 100.0, 1.0, true);
    let c_last = make_1m_candle(3_540_000, 100.0, 100.0, 100.0, 100.0, 1.0, true);
    assert!(!Aggregator::is_last_1m_in_group(&c_mid, Timeframe::H1));
    assert!(Aggregator::is_last_1m_in_group(&c_last, Timeframe::H1));

    // 1d assertions (merged from test_is_last_1m_in_group_1d)
    let c_day_last = make_1m_candle(86_340_000, 100.0, 100.0, 100.0, 100.0, 1.0, true);
    assert!(Aggregator::is_last_1m_in_group(&c_day_last, Timeframe::D1));
}


#[test]
fn test_update_higher_timeframes_first_candle() {
    let mut cache = SymbolCache::new();
    let candle = make_1m_candle(0, 100.0, 105.0, 99.0, 102.0, 10.0, false);

    let updates = Aggregator::update_higher_timeframes(&candle, &mut cache);
    assert_eq!(updates.len(), 5);

    for (tf, c) in &updates {
        assert_eq!(c.open_time, 0);
        assert_eq!(c.open, 100.0);
        assert!(!c.closed, "Timeframe {} should be unclosed", tf.as_str());
    }
}

#[test]
fn test_update_higher_timeframes_closing_5m() {
    let mut cache = SymbolCache::new();
    for i in 0..4 {
        let t = i as i64 * 60_000;
        let c = make_1m_candle(t, 100.0, 105.0, 99.0, 102.0, 10.0, true);
        cache.update_candle(Timeframe::M1, c.clone());
        Aggregator::update_higher_timeframes(&c, &mut cache);
    }

    let last_1m = make_1m_candle(240_000, 102.0, 108.0, 101.0, 106.0, 12.0, true);
    let updates = Aggregator::update_higher_timeframes(&last_1m, &mut cache);

    let m5_update = updates.iter().find(|(tf, _)| *tf == Timeframe::M5);
    assert!(m5_update.is_some());
    let (_, m5_candle) = m5_update.unwrap();
    assert!(m5_candle.closed);
}

#[test]
fn test_update_higher_timeframes_new_period() {
    let mut cache = SymbolCache::new();
    let c1 = make_1m_candle(0, 100.0, 105.0, 99.0, 102.0, 10.0, true);
    cache.update_candle(Timeframe::M1, c1.clone());
    Aggregator::update_higher_timeframes(&c1, &mut cache);

    let c2 = make_1m_candle(300_000, 110.0, 115.0, 109.0, 112.0, 10.0, true);
    let updates = Aggregator::update_higher_timeframes(&c2, &mut cache);

    let m5_update = updates.iter().find(|(tf, _)| *tf == Timeframe::M5);
    assert!(m5_update.is_some());
    let (_, m5_candle) = m5_update.unwrap();
    assert_eq!(m5_candle.open_time, 300_000);
    assert_eq!(m5_candle.open, 110.0);
}

#[test]
fn test_aggregate_non_aligned_start() {
    let candles = make_1m_sequence(120_000, 8, 100.0);
    let result = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::M5);
    assert!(!result.is_empty());
    assert_eq!(result[0].open_time, 0);
    assert_eq!(result[0].open, 100.0);
}

#[test]
fn test_aggregate_gap_in_1m_data() {
    let mut candles = make_1m_sequence(0, 3, 100.0);
    candles.extend(make_1m_sequence(600_000, 3, 110.0));
    let result = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::M5);
    assert!(result.len() >= 2);
    assert_eq!(result[0].open_time, 0);
    assert_eq!(result[1].open_time, 600_000);
}

// ============================================================
// P0-5: update_higher_timeframes 旧数据分支测试 (1 test)
// ============================================================

/// P0-5: When a 1m candle's aligned open time is older than the last
/// existing higher-tf candle, merge_into_timeframe returns None (stale data branch).
#[test]
fn test_update_higher_timeframes_stale_data_branch() {
    let mut cache = SymbolCache::new();

    // Place a closed 5m candle at open_time=300_000 (the second 5m group)
    let existing_5m = make_1m_candle(300_000, 110.0, 115.0, 109.0, 112.0, 50.0, true);
    cache.update_candle(Timeframe::M5, existing_5m);

    // Now push a 1m candle whose 5m aligned time (0) is BEFORE the existing 5m (300_000).
    // This triggers the `aligned_open < last_candle.open_time` branch.
    let stale_1m = make_1m_candle(0, 100.0, 105.0, 99.0, 102.0, 10.0, true);
    let updates = Aggregator::update_higher_timeframes(&stale_1m, &mut cache);

    // The M5 entry in updates should be None (stale data ignored)
    let m5_update = updates.iter().find(|(tf, _)| *tf == Timeframe::M5);
    assert!(m5_update.is_none(), "stale 1m data should not produce a M5 update");

    // The existing 5m candle should remain unchanged
    let m5_candles = cache.get_klines(Timeframe::M5);
    assert_eq!(m5_candles.len(), 1);
    assert_eq!(m5_candles[0].open_time, 300_000);
    assert_eq!(m5_candles[0].open, 110.0);
}
