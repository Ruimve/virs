use crate::models::Kline;
use std::collections::HashMap;
use talib_rs::{ma_type::MaType, math_operator, momentum, overlap, volatility, volume};

pub fn closes(klines: &[Kline]) -> Vec<f64> {
    klines.iter().map(|k| k.close).collect()
}

pub fn highs(klines: &[Kline]) -> Vec<f64> {
    klines.iter().map(|k| k.high).collect()
}

pub fn lows(klines: &[Kline]) -> Vec<f64> {
    klines.iter().map(|k| k.low).collect()
}

pub fn volumes(klines: &[Kline]) -> Vec<f64> {
    klines.iter().map(|k| k.volume).collect()
}

pub fn sma(klines: &[Kline], period: usize) -> Vec<f64> {
    overlap::sma(&closes(klines), period).unwrap_or_default()
}

pub fn ema(klines: &[Kline], period: usize) -> Vec<f64> {
    overlap::ema(&closes(klines), period).unwrap_or_default()
}

pub fn rsi(klines: &[Kline], period: usize) -> Vec<f64> {
    momentum::rsi(&closes(klines), period).unwrap_or_default()
}

pub fn macd(
    klines: &[Kline],
    fast: usize,
    slow: usize,
    signal: usize,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    momentum::macd(&closes(klines), fast, slow, signal).unwrap_or_default()
}

pub fn bollinger_bands(
    klines: &[Kline],
    period: usize,
    std_dev: f64,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    overlap::bbands(&closes(klines), period, std_dev, std_dev, MaType::Sma).unwrap_or_default()
}

pub fn atr(klines: &[Kline], period: usize) -> Vec<f64> {
    volatility::atr(&highs(klines), &lows(klines), &closes(klines), period).unwrap_or_default()
}

pub fn stoch(klines: &[Kline], k_period: usize, d_period: usize) -> (Vec<f64>, Vec<f64>) {
    momentum::stoch(
        &highs(klines),
        &lows(klines),
        &closes(klines),
        k_period,
        d_period,
        MaType::Sma,
        d_period,
        MaType::Sma,
    )
    .unwrap_or_default()
}

pub fn adx(klines: &[Kline], period: usize) -> Vec<f64> {
    momentum::adx(&highs(klines), &lows(klines), &closes(klines), period).unwrap_or_default()
}

pub fn cci(klines: &[Kline], period: usize) -> Vec<f64> {
    momentum::cci(&highs(klines), &lows(klines), &closes(klines), period).unwrap_or_default()
}

pub fn willr(klines: &[Kline], period: usize) -> Vec<f64> {
    momentum::willr(&highs(klines), &lows(klines), &closes(klines), period).unwrap_or_default()
}

pub fn mfi(klines: &[Kline], period: usize) -> Vec<f64> {
    momentum::mfi(
        &highs(klines),
        &lows(klines),
        &closes(klines),
        &volumes(klines),
        period,
    )
    .unwrap_or_default()
}

pub fn obv(klines: &[Kline]) -> Vec<f64> {
    volume::obv(&closes(klines), &volumes(klines)).unwrap_or_default()
}

pub fn highest(klines: &[Kline], period: usize) -> Vec<f64> {
    math_operator::max(&highs(klines), period).unwrap_or_default()
}

pub fn lowest(klines: &[Kline], period: usize) -> Vec<f64> {
    math_operator::min(&lows(klines), period).unwrap_or_default()
}

pub fn highest_close(klines: &[Kline], period: usize) -> Vec<f64> {
    math_operator::max(&closes(klines), period).unwrap_or_default()
}

pub fn lowest_close(klines: &[Kline], period: usize) -> Vec<f64> {
    math_operator::min(&closes(klines), period).unwrap_or_default()
}

#[inline(always)]
pub fn sma_at(klines: &[Kline], idx: usize, period: usize) -> f64 {
    if klines.is_empty() || idx < period - 1 || period == 0 {
        return 0.0;
    }
    let result = overlap::sma(&closes(klines), period).unwrap_or_default();
    result.get(idx).copied().unwrap_or(0.0)
}

#[allow(dead_code)]
#[inline(always)]
pub fn ema_at(klines: &[Kline], idx: usize, period: usize) -> f64 {
    if klines.is_empty() || idx < period - 1 || period == 0 {
        return 0.0;
    }
    let result = overlap::ema(&closes(klines), period).unwrap_or_default();
    result.get(idx).copied().unwrap_or(0.0)
}

#[inline(always)]
pub fn rsi_at(klines: &[Kline], idx: usize, period: usize) -> f64 {
    if klines.is_empty() || idx < period || period == 0 {
        return 50.0;
    }
    let result = momentum::rsi(&closes(klines), period).unwrap_or_default();
    result.get(idx).copied().unwrap_or(50.0)
}

#[inline(always)]
pub fn macd_at(klines: &[Kline], idx: usize, fast: usize, slow: usize) -> f64 {
    if klines.is_empty() || idx < slow - 1 {
        return 0.0;
    }
    let (macd, _, _) = momentum::macd(&closes(klines), fast, slow, 9).unwrap_or_default();
    macd.get(idx).copied().unwrap_or(0.0)
}

#[inline(always)]
pub fn macd_signal_at(
    klines: &[Kline],
    idx: usize,
    fast: usize,
    slow: usize,
    signal: usize,
) -> f64 {
    if klines.is_empty() || idx < slow + signal - 2 {
        return 0.0;
    }
    let (_, sig, _) = momentum::macd(&closes(klines), fast, slow, signal).unwrap_or_default();
    sig.get(idx).copied().unwrap_or(0.0)
}

#[inline(always)]
pub fn bbands_at(
    klines: &[Kline],
    idx: usize,
    period: usize,
    std_dev: f64,
) -> (f64, f64, f64) {
    if klines.is_empty() || idx < period - 1 {
        return (0.0, 0.0, 0.0);
    }
    let (upper, middle, lower) =
        overlap::bbands(&closes(klines), period, std_dev, std_dev, MaType::Sma).unwrap_or_default();
    (
        upper.get(idx).copied().unwrap_or(0.0),
        middle.get(idx).copied().unwrap_or(0.0),
        lower.get(idx).copied().unwrap_or(0.0),
    )
}

#[allow(dead_code)]
#[inline(always)]
pub fn atr_at(klines: &[Kline], idx: usize, period: usize) -> f64 {
    if klines.is_empty() || idx < period {
        return 0.0;
    }
    let result =
        volatility::atr(&highs(klines), &lows(klines), &closes(klines), period).unwrap_or_default();
    result.get(idx).copied().unwrap_or(0.0)
}

#[allow(dead_code)]
#[inline(always)]
pub fn stoch_at(
    klines: &[Kline],
    idx: usize,
    k_period: usize,
    d_period: usize,
) -> (f64, f64) {
    if klines.is_empty() || idx < k_period + d_period - 1 {
        return (50.0, 50.0);
    }
    let (slowk, slowd) = momentum::stoch(
        &highs(klines),
        &lows(klines),
        &closes(klines),
        k_period,
        d_period,
        MaType::Sma,
        d_period,
        MaType::Sma,
    )
    .unwrap_or_default();
    (
        slowk.get(idx).copied().unwrap_or(50.0),
        slowd.get(idx).copied().unwrap_or(50.0),
    )
}

pub struct PrecomputedIndicators {
    len: usize,
    sma_cache: HashMap<usize, Vec<f64>>,
    ema_cache: HashMap<usize, Vec<f64>>,
    rsi_cache: HashMap<usize, Vec<f64>>,
    macd_cache: HashMap<(usize, usize, usize), (Vec<f64>, Vec<f64>, Vec<f64>)>,
    bbands_cache: HashMap<(usize, i64), (Vec<f64>, Vec<f64>, Vec<f64>)>,
    atr_cache: HashMap<usize, Vec<f64>>,
    stoch_cache: HashMap<(usize, usize), (Vec<f64>, Vec<f64>)>,
}

impl PrecomputedIndicators {
    pub fn new(klines: &[Kline]) -> Self {
        Self {
            len: klines.len(),
            sma_cache: HashMap::new(),
            ema_cache: HashMap::new(),
            rsi_cache: HashMap::new(),
            macd_cache: HashMap::new(),
            bbands_cache: HashMap::new(),
            atr_cache: HashMap::new(),
            stoch_cache: HashMap::new(),
        }
    }

    pub fn sma(&mut self, klines: &[Kline], period: usize) -> &[f64] {
        self.sma_cache
            .entry(period)
            .or_insert_with(|| sma(klines, period))
    }

    pub fn ema(&mut self, klines: &[Kline], period: usize) -> &[f64] {
        self.ema_cache
            .entry(period)
            .or_insert_with(|| ema(klines, period))
    }

    pub fn rsi(&mut self, klines: &[Kline], period: usize) -> &[f64] {
        self.rsi_cache
            .entry(period)
            .or_insert_with(|| rsi(klines, period))
    }

    pub fn macd(
        &mut self,
        klines: &[Kline],
        fast: usize,
        slow: usize,
        signal: usize,
    ) -> (&[f64], &[f64], &[f64]) {
        self.macd_cache
            .entry((fast, slow, signal))
            .or_insert_with(|| macd(klines, fast, slow, signal));
        let (ref m, ref s, ref h) = self.macd_cache[&(fast, slow, signal)];
        (m, s, h)
    }

    pub fn bbands(&mut self, klines: &[Kline], period: usize, std_dev: f64) -> (&[f64], &[f64], &[f64]) {
        let key = (period, (std_dev * 1000.0) as i64);
        self.bbands_cache
            .entry(key)
            .or_insert_with(|| bollinger_bands(klines, period, std_dev));
        let (ref u, ref m, ref l) = self.bbands_cache[&key];
        (u, m, l)
    }

    pub fn atr(&mut self, klines: &[Kline], period: usize) -> &[f64] {
        self.atr_cache
            .entry(period)
            .or_insert_with(|| atr(klines, period))
    }

    pub fn stoch(&mut self, klines: &[Kline], k_period: usize, d_period: usize) -> (&[f64], &[f64]) {
        self.stoch_cache
            .entry((k_period, d_period))
            .or_insert_with(|| stoch(klines, k_period, d_period));
        let (ref k, ref d) = self.stoch_cache[&(k_period, d_period)];
        (k, d)
    }

    #[inline(always)]
    pub fn sma_at(&self, idx: usize, period: usize) -> f64 {
        self.sma_cache
            .get(&period)
            .and_then(|v| v.get(idx).copied())
            .unwrap_or(0.0)
    }

    #[inline(always)]
    pub fn ema_at(&self, idx: usize, period: usize) -> f64 {
        self.ema_cache
            .get(&period)
            .and_then(|v| v.get(idx).copied())
            .unwrap_or(0.0)
    }

    #[inline(always)]
    pub fn rsi_at(&self, idx: usize, period: usize) -> f64 {
        self.rsi_cache
            .get(&period)
            .and_then(|v| v.get(idx).copied())
            .unwrap_or(50.0)
    }

    #[inline(always)]
    pub fn macd_line_at(&self, idx: usize, fast: usize, slow: usize, signal: usize) -> f64 {
        self.macd_cache
            .get(&(fast, slow, signal))
            .and_then(|(m, _, _)| m.get(idx).copied())
            .unwrap_or(0.0)
    }

    #[inline(always)]
    pub fn macd_signal_at(&self, idx: usize, fast: usize, slow: usize, signal: usize) -> f64 {
        self.macd_cache
            .get(&(fast, slow, signal))
            .and_then(|(_, s, _)| s.get(idx).copied())
            .unwrap_or(0.0)
    }

    #[inline(always)]
    pub fn bbands_at(&self, idx: usize, period: usize, std_dev: f64) -> (f64, f64, f64) {
        let key = (period, (std_dev * 1000.0) as i64);
        self.bbands_cache
            .get(&key)
            .map(|(u, m, l)| {
                (
                    u.get(idx).copied().unwrap_or(0.0),
                    m.get(idx).copied().unwrap_or(0.0),
                    l.get(idx).copied().unwrap_or(0.0),
                )
            })
            .unwrap_or((0.0, 0.0, 0.0))
    }

    #[inline(always)]
    pub fn atr_at(&self, idx: usize, period: usize) -> f64 {
        self.atr_cache
            .get(&period)
            .and_then(|v| v.get(idx).copied())
            .unwrap_or(0.0)
    }

    #[inline(always)]
    pub fn stoch_at(&self, idx: usize, k_period: usize, d_period: usize) -> (f64, f64) {
        self.stoch_cache
            .get(&(k_period, d_period))
            .map(|(k, d)| {
                (
                    k.get(idx).copied().unwrap_or(50.0),
                    d.get(idx).copied().unwrap_or(50.0),
                )
            })
            .unwrap_or((50.0, 50.0))
    }
}
