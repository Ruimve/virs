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

    pub(crate) fn is_last_1m_in_group(candle_1m: &Candle, tf: Timeframe) -> bool {
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

