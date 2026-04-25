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

    #[test]
    fn test_aggregate_5m() {
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
        let mut candles = vec![];
        for i in 0..10 {
            let t = i as i64 * 60_000;
            candles.push(make_1m_candle(t, 100.0 + i as f64, 105.0, 99.0, 100.0 + i as f64, 10.0, true));
        }

        let result = Aggregator::aggregate_1m_to_timeframe(&candles, Timeframe::M5);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].open_time, 0);
        assert_eq!(result[1].open_time, 300_000);
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
}
