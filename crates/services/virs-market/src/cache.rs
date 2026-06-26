//! Symbol cache — per-symbol multi-timeframe candle buffer.

use std::collections::{HashMap, VecDeque};

use super::types::{AllTimeframesData, Candle, Timeframe};

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

    fn last(&self) -> Option<&Candle> {
        self.candles.back()
    }

    fn get_all(&self) -> Vec<Candle> {
        self.candles.iter().cloned().collect()
    }

    fn len(&self) -> usize {
        self.candles.len()
    }

    fn is_empty(&self) -> bool {
        self.candles.is_empty()
    }

    fn replace_all(&mut self, candles: Vec<Candle>) {
        self.candles = candles.into_iter().collect();
        while self.candles.len() > self.max_size {
            self.candles.pop_front();
        }
    }

    fn insert_backfilled(&mut self, candles: Vec<Candle>) {
        if self.candles.is_empty() {
            self.replace_all(candles);
            return;
        }
        let last_existing_time = self.candles.back().map(|c| c.open_time).unwrap_or(0);
        for c in candles {
            if c.open_time > last_existing_time {
                self.candles.push_back(c);
            } else if let Some(existing) =
                self.candles.iter_mut().find(|e| e.open_time == c.open_time)
            {
                if c.closed && !existing.closed {
                    *existing = c;
                }
            }
        }
        while self.candles.len() > self.max_size {
            self.candles.pop_front();
        }
    }
}

pub struct SymbolCache {
    timeframes: HashMap<Timeframe, TimeframeBuffer>,
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

    pub fn get_all_timeframes(&self) -> AllTimeframesData {
        AllTimeframesData {
            m1: self.get_klines(Timeframe::M1),
            m5: self.get_klines(Timeframe::M5),
            m15: self.get_klines(Timeframe::M15),
            h1: self.get_klines(Timeframe::H1),
            h4: self.get_klines(Timeframe::H4),
            d1: self.get_klines(Timeframe::D1),
        }
    }

    pub fn last_closed_1m(&self) -> Option<Candle> {
        self.timeframes
            .get(&Timeframe::M1)
            .and_then(|buf| buf.last_closed())
            .cloned()
    }

    pub fn last_1m(&self) -> Option<Candle> {
        self.timeframes
            .get(&Timeframe::M1)
            .and_then(|buf| buf.last())
            .cloned()
    }

    pub fn replace_timeframe(&mut self, timeframe: Timeframe, candles: Vec<Candle>) {
        if let Some(buf) = self.timeframes.get_mut(&timeframe) {
            buf.replace_all(candles);
        }
    }

    pub fn backfill_timeframe(&mut self, timeframe: Timeframe, candles: Vec<Candle>) {
        if let Some(buf) = self.timeframes.get_mut(&timeframe) {
            buf.insert_backfilled(candles);
        }
    }

    pub fn candle_count(&self, timeframe: Timeframe) -> usize {
        self.timeframes
            .get(&timeframe)
            .map(|buf| buf.len())
            .unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.timeframes.values().all(|buf| buf.is_empty())
    }
}
