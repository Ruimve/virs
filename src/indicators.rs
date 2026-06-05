use crate::models::Kline;
use talib_rs::{ma_type::MaType, math_operator, momentum, overlap, volatility};

pub fn closes(klines: &[Kline]) -> Vec<f64> {
    klines.iter().map(|k| k.close).collect()
}

pub fn highs(klines: &[Kline]) -> Vec<f64> {
    klines.iter().map(|k| k.high).collect()
}

pub fn lows(klines: &[Kline]) -> Vec<f64> {
    klines.iter().map(|k| k.low).collect()
}

pub fn atr(klines: &[Kline], period: usize) -> Vec<f64> {
    volatility::atr(&highs(klines), &lows(klines), &closes(klines), period).unwrap_or_default()
}

pub fn sma_at_from(series: &[f64], idx: usize, period: usize) -> f64 {
    if series.is_empty() || period == 0 {
        return 0.0;
    }
    let nan_count = series.iter().take(idx + 1).filter(|v| v.is_nan()).count();
    let valid: Vec<f64> = series.iter().filter(|v| !v.is_nan()).copied().collect();
    if valid.len() < period {
        if valid.is_empty() { return 0.0; }
        return valid.iter().rev().take(period.min(valid.len())).sum::<f64>() / period.min(valid.len()) as f64;
    }
    let mapped_idx = idx.saturating_sub(nan_count);
    let result = overlap::sma(&valid, period).unwrap_or_default();
    result.get(mapped_idx).copied().unwrap_or_else(|| {
        result.last().copied().unwrap_or(0.0)
    })
}

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

#[inline(always)]
pub fn atr_at(klines: &[Kline], idx: usize, period: usize) -> f64 {
    if klines.is_empty() || idx < period {
        return 0.0;
    }
    let result =
        volatility::atr(&highs(klines), &lows(klines), &closes(klines), period).unwrap_or_default();
    result.get(idx).copied().unwrap_or(0.0)
}

#[inline(always)]
pub fn adx_at(klines: &[Kline], idx: usize, period: usize) -> f64 {
    if klines.is_empty() || idx < period * 2 {
        return 0.0;
    }
    let result = momentum::adx(&highs(klines), &lows(klines), &closes(klines), period).unwrap_or_default();
    result.get(idx).copied().unwrap_or(0.0)
}

#[inline(always)]
pub fn macd_histogram_at(
    klines: &[Kline], idx: usize, fast: usize, slow: usize, signal: usize,
) -> f64 {
    if klines.is_empty() || idx < slow + signal - 2 {
        return 0.0;
    }
    let (macd, sig, _) = momentum::macd(&closes(klines), fast, slow, signal).unwrap_or_default();
    let m = macd.get(idx).copied().unwrap_or(0.0);
    let s = sig.get(idx).copied().unwrap_or(0.0);
    m - s
}

#[inline(always)]
pub fn bbands_width_at(
    klines: &[Kline], idx: usize, period: usize, std_dev: f64,
) -> f64 {
    let (upper, middle, lower) = bbands_at(klines, idx, period, std_dev);
    if middle == 0.0 { return 0.0; }
    (upper - lower) / middle
}

#[inline(always)]
pub fn highest_at(klines: &[Kline], idx: usize, period: usize) -> f64 {
    if klines.is_empty() || idx < period - 1 || period == 0 {
        return 0.0;
    }
    let result = math_operator::max(&highs(klines), period).unwrap_or_default();
    result.get(idx).copied().unwrap_or(0.0)
}

#[inline(always)]
pub fn lowest_at(klines: &[Kline], idx: usize, period: usize) -> f64 {
    if klines.is_empty() || idx < period - 1 || period == 0 {
        return 0.0;
    }
    let result = math_operator::min(&lows(klines), period).unwrap_or_default();
    result.get(idx).copied().unwrap_or(0.0)
}

pub fn volume_sma_at(klines: &[Kline], idx: usize, period: usize) -> f64 {
    if idx < period - 1 || klines.is_empty() {
        return 0.0;
    }
    let start = idx + 1 - period;
    let sum: f64 = (start..=idx).map(|i| klines[i].volume).sum();
    sum / period as f64
}

/// Count consecutive bars outside Bollinger Band.
/// Returns positive count for bars above upper band, negative for below lower band.
pub fn compute_bars_outside_band(klines: &[Kline], bb_upper: f64, bb_lower: f64) -> i32 {
    let mut count: i32 = 0;
    for k in klines.iter().rev() {
        if k.close > bb_upper {
            count += 1;
        } else if k.close < bb_lower {
            count -= 1;
        } else {
            break;
        }
    }
    count
}

/// Find nearest round number (support/resistance level).
/// upward = true: find next resistance above price
/// upward = false: find next support below price
pub fn find_round_number(price: f64, upward: bool) -> f64 {
    if price <= 0.0 { return 0.0; }
    let magnitude = 10_f64.powf(price.log10().floor());
    let step = if magnitude >= 10000.0 { 1000.0 } else if magnitude >= 1000.0 { 100.0 } else if magnitude >= 100.0 { 10.0 } else if magnitude >= 10.0 { 5.0 } else { 1.0 };
    if upward {
        (price / step).ceil() * step
    } else {
        (price / step).floor() * step
    }
}
