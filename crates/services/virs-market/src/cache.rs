use std::collections::{HashMap, VecDeque};

use virs_types::{Candle, Timeframe};

struct TimeframeBuffer {
    candles: VecDeque<Candle>,
    max_size: usize,
}

impl TimeframeBuffer {
    fn new(max_size: usize) -> Self {
        Self {
            candles: VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    fn push_or_update(&mut self, candle: Candle) {
        if let Some(last) = self.candles.back_mut() {
            if last.open_time == candle.open_time {
                *last = candle;
                return;
            }
            if candle.open_time < last.open_time {
                if let Some(existing) = self
                    .candles
                    .iter_mut()
                    .find(|c| c.open_time == candle.open_time)
                {
                    *existing = candle;
                    return;
                }
            }
        }
        self.candles.push_back(candle);
        while self.candles.len() > self.max_size {
            self.candles.pop_front();
        }
    }

    fn close_candle(&mut self, open_time: i64) {
        if let Some(c) = self.candles.iter_mut().find(|c| c.open_time == open_time) {
            c.closed = true;
        }
    }

    fn last_closed(&self) -> Option<&Candle> {
        self.candles.iter().rev().find(|c| c.closed)
    }

    fn get_all(&self) -> Vec<Candle> {
        self.candles.iter().cloned().collect()
    }

    fn replace_all(&mut self, candles: Vec<Candle>) {
        self.candles = candles.into_iter().collect();
        while self.candles.len() > self.max_size {
            self.candles.pop_front();
        }
    }
}

pub struct SymbolCache {
    timeframes: HashMap<Timeframe, TimeframeBuffer>,
}

impl Default for SymbolCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolCache {
    pub fn new() -> Self {
        let mut timeframes = HashMap::new();
        for tf in Timeframe::all() {
            timeframes.insert(*tf, TimeframeBuffer::new(tf.default_limit()));
        }
        Self { timeframes }
    }

    pub fn update_candle(&mut self, timeframe: Timeframe, candle: Candle) {
        if let Some(buf) = self.timeframes.get_mut(&timeframe) {
            buf.push_or_update(candle);
        }
    }

    pub fn close_candle(&mut self, timeframe: Timeframe, open_time: i64) {
        if let Some(buf) = self.timeframes.get_mut(&timeframe) {
            buf.close_candle(open_time);
        }
    }

    pub fn get_klines(&self, timeframe: Timeframe) -> Vec<Candle> {
        self.timeframes
            .get(&timeframe)
            .map(|buf| buf.get_all())
            .unwrap_or_default()
    }

    pub fn last_closed_1m(&self) -> Option<Candle> {
        self.timeframes
            .get(&Timeframe::M1)
            .and_then(|buf| buf.last_closed())
            .cloned()
    }

    pub fn replace_timeframe(&mut self, timeframe: Timeframe, candles: Vec<Candle>) {
        if let Some(buf) = self.timeframes.get_mut(&timeframe) {
            buf.replace_all(candles);
        }
    }
}
