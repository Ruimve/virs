use super::types::{Candle, Timeframe, align_open_time};

pub struct Aggregator;

impl Aggregator {
    pub fn update_higher_timeframes(
        candle_1m: &Candle,
        cache: &mut super::cache::SymbolCache,
    ) -> Vec<(Timeframe, Candle)> {
        let mut updated = Vec::new();

        for &tf in &[Timeframe::M5, Timeframe::M15, Timeframe::H1, Timeframe::H4, Timeframe::D1] {
            if let Some(merged) = Self::merge_into_timeframe(candle_1m, tf, cache) {
                updated.push((tf, merged));
            }
        }

        updated
    }

    fn merge_into_timeframe(
        candle_1m: &Candle,
        tf: Timeframe,
        cache: &mut super::cache::SymbolCache,
    ) -> Option<Candle> {
        let aligned_open = align_open_time(candle_1m.open_time, tf);
        let tf_ms = tf.ms();
        let close_time = aligned_open + tf_ms - 1;

        let existing = cache.get_klines(tf);
        let last = existing.last();

        let is_closing = Self::is_last_1m_in_group(candle_1m, tf);

        match last {
            Some(last_candle) if last_candle.open_time == aligned_open => {
                let mut merged = last_candle.clone();
                if candle_1m.high > merged.high {
                    merged.high = candle_1m.high;
                }
                if candle_1m.low < merged.low {
                    merged.low = candle_1m.low;
                }
                merged.close = candle_1m.close;
                merged.volume += candle_1m.volume;
                merged.quote_volume += candle_1m.quote_volume;
                merged.trades += candle_1m.trades;
                merged.close_time = close_time;
                merged.closed = is_closing && candle_1m.closed;

                let result = merged.clone();
                cache.update_candle(tf, merged);
                Some(result)
            }
            Some(last_candle) if aligned_open > last_candle.open_time => {
                let mut new_candle = Candle::from_1m(candle_1m, tf);
                new_candle.close_time = close_time;
                new_candle.closed = is_closing && candle_1m.closed;

                let result = new_candle.clone();
                cache.update_candle(tf, new_candle);
                Some(result)
            }
            Some(last_candle) if aligned_open < last_candle.open_time => {
                None
            }
            None => {
                let mut new_candle = Candle::from_1m(candle_1m, tf);
                new_candle.close_time = close_time;
                new_candle.closed = is_closing && candle_1m.closed;

                let result = new_candle.clone();
                cache.update_candle(tf, new_candle);
                Some(result)
            }
            _ => None,
        }
    }

    fn is_last_1m_in_group(candle_1m: &Candle, tf: Timeframe) -> bool {
        let tf_ms = tf.ms();
        let aligned_open = align_open_time(candle_1m.open_time, tf);
        let group_end = aligned_open + tf_ms - 1;
        let candle_1m_end = candle_1m.open_time + 60_000 - 1;
        candle_1m_end >= group_end
    }

    pub fn aggregate_1m_to_timeframe(candles_1m: &[Candle], tf: Timeframe) -> Vec<Candle> {
        if candles_1m.is_empty() {
            return Vec::new();
        }

        let tf_ms = tf.ms();
        let mut result: Vec<Candle> = Vec::new();
        let mut current: Option<Candle> = None;

        for c in candles_1m {
            let aligned_open = align_open_time(c.open_time, tf);
            let close_time = aligned_open + tf_ms - 1;
            let is_closing = Self::is_last_1m_in_group(c, tf);

            match &mut current {
                Some(curr) if curr.open_time == aligned_open => {
                    if c.high > curr.high {
                        curr.high = c.high;
                    }
                    if c.low < curr.low {
                        curr.low = c.low;
                    }
                    curr.close = c.close;
                    curr.volume += c.volume;
                    curr.quote_volume += c.quote_volume;
                    curr.trades += c.trades;
                    curr.close_time = close_time;
                    if is_closing && c.closed {
                        curr.closed = true;
                    }
                }
                Some(curr) => {
                    result.push(curr.clone());
                    let mut new_c = Candle::from_1m(c, tf);
                    new_c.close_time = close_time;
                    new_c.closed = is_closing && c.closed;
                    current = Some(new_c);
                }
                None => {
                    let mut new_c = Candle::from_1m(c, tf);
                    new_c.close_time = close_time;
                    new_c.closed = is_closing && c.closed;
                    current = Some(new_c);
                }
            }
        }

        if let Some(c) = current {
            result.push(c);
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::cache::SymbolCache;

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
    fn test_aggregate_high_low_tracking() {
        let candles = vec![
            make_1m_candle(0, 100.0, 105.0, 98.0, 102.0, 10.0, true),
            make_1m_candle(60_000, 102.0, 120.0, 101.0, 106.0, 12.0, true),
            make_1m_candle(120_000, 106.0, 108.0, 90.0, 107.0, 8.0, true),
            make_1m_candle(180_000, 107.0, 112.0, 106.0, 109.0, 15.0, true),
            make_1m_candle(240_000, 109.0, 115.0, 108.0, 111.0, 11.0, true),
        ];
        let result = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::M5);
        assert_eq!(result[0].high, 120.0);
        assert_eq!(result[0].low, 90.0);
    }

    #[test]
    fn test_aggregate_volume_accumulation() {
        let candles = vec![
            make_1m_candle(0, 100.0, 105.0, 99.0, 102.0, 10.0, true),
            make_1m_candle(60_000, 102.0, 108.0, 101.0, 106.0, 20.0, true),
            make_1m_candle(120_000, 106.0, 110.0, 104.0, 107.0, 30.0, true),
            make_1m_candle(180_000, 107.0, 112.0, 106.0, 109.0, 40.0, true),
            make_1m_candle(240_000, 109.0, 115.0, 108.0, 111.0, 50.0, true),
        ];
        let result = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::M5);
        assert!((result[0].volume - 150.0).abs() < 0.001);
        assert_eq!(result[0].trades, 500);
    }

    #[test]
    fn test_is_last_1m_in_group() {
        let c1 = make_1m_candle(0, 100.0, 100.0, 100.0, 100.0, 1.0, true);
        let c2 = make_1m_candle(240_000, 100.0, 100.0, 100.0, 100.0, 1.0, true);
        let c3 = make_1m_candle(300_000, 100.0, 100.0, 100.0, 100.0, 1.0, true);

        assert!(!Aggregator::is_last_1m_in_group(&c1, Timeframe::M5));
        assert!(Aggregator::is_last_1m_in_group(&c2, Timeframe::M5));
        assert!(!Aggregator::is_last_1m_in_group(&c3, Timeframe::M5));
    }

    #[test]
    fn test_is_last_1m_in_group_1h() {
        let c_mid = make_1m_candle(1_800_000, 100.0, 100.0, 100.0, 100.0, 1.0, true);
        let c_last = make_1m_candle(3_540_000, 100.0, 100.0, 100.0, 100.0, 1.0, true);
        assert!(!Aggregator::is_last_1m_in_group(&c_mid, Timeframe::H1));
        assert!(Aggregator::is_last_1m_in_group(&c_last, Timeframe::H1));
    }

    #[test]
    fn test_is_last_1m_in_group_1d() {
        let c_last = make_1m_candle(86_340_000, 100.0, 100.0, 100.0, 100.0, 1.0, true);
        assert!(Aggregator::is_last_1m_in_group(&c_last, Timeframe::D1));
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
}
