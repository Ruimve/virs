//! 原子指标计算函数（TA-Lib 封装 + 手写辅助）。
//!
//! 本模块从 `common/indicators.rs` 原样迁移，保持计算行为完全一致。
//!
//! **调用方契约**：本模块的指标函数在 K 线数据不足或 TA-Lib 计算失败时返回 `Err`，
//! 不再使用 `0.0`/`50.0` 兜底。调用方（如 [`IndicatorSet::compute`](super::set::IndicatorSet::compute)）
//! 应通过 `?` 传播错误。

use talib_rs::{ma_type::MaType, math_operator, momentum, overlap, volatility};
use virs_error::{Context, VirsError, VirsResult};
use virs_types::Kline;

pub fn closes(klines: &[Kline]) -> Vec<f64> {
    klines.iter().map(|k| k.close).collect()
}

pub fn highs(klines: &[Kline]) -> Vec<f64> {
    klines.iter().map(|k| k.high).collect()
}

pub fn lows(klines: &[Kline]) -> Vec<f64> {
    klines.iter().map(|k| k.low).collect()
}

pub fn atr(klines: &[Kline], period: usize) -> VirsResult<Vec<f64>> {
    Ok(volatility::atr(&highs(klines), &lows(klines), &closes(klines), period)
        .context("indicator atr: TA-Lib ATR calculation failed")?)
}

pub fn sma_at_from(series: &[f64], idx: usize, period: usize) -> VirsResult<f64> {
    if series.is_empty() || period == 0 {
        return Err(VirsError::config(format!(
            "indicator sma_at_from: insufficient data at idx={idx} (empty series or period=0, period={period})"
        )));
    }
    let nan_count = series.iter().take(idx + 1).filter(|v| v.is_nan()).count();
    let valid: Vec<f64> = series.iter().filter(|v| !v.is_nan()).copied().collect();
    if valid.len() < period {
        if valid.is_empty() {
            return Err(VirsError::config(format!(
                "indicator sma_at_from: insufficient data at idx={idx} (no valid values, period={period})"
            )));
        }
        return Ok(valid
            .iter()
            .rev()
            .take(period.min(valid.len()))
            .sum::<f64>()
            / period.min(valid.len()) as f64);
    }
    let mapped_idx = idx.saturating_sub(nan_count);
    let result = overlap::sma(&valid, period)
        .context("indicator sma_at_from: TA-Lib SMA calculation failed")?;
    result
        .get(mapped_idx)
        .copied()
        .or_else(|| result.last().copied())
        .ok_or_else(|| {
            VirsError::config(format!(
                "indicator sma_at_from: insufficient data at idx={idx} (no result produced, period={period})"
            ))
        })
}

#[inline(always)]
pub fn ema_at(klines: &[Kline], idx: usize, period: usize) -> VirsResult<f64> {
    if klines.is_empty() || idx < period - 1 || period == 0 {
        return Err(VirsError::config(format!(
            "indicator ema_at: insufficient data at idx={idx} (period={period})"
        )));
    }
    let result = overlap::ema(&closes(klines), period)
        .context("indicator ema_at: TA-Lib EMA calculation failed")?;
    result.get(idx).copied().ok_or_else(|| {
        VirsError::config(format!(
            "indicator ema_at: insufficient data at idx={idx} (no result produced, period={period})"
        ))
    })
}

#[inline(always)]
pub fn rsi_at(klines: &[Kline], idx: usize, period: usize) -> VirsResult<f64> {
    if klines.is_empty() || idx < period || period == 0 {
        return Err(VirsError::config(format!(
            "indicator rsi_at: insufficient data at idx={idx} (period={period})"
        )));
    }
    let result = momentum::rsi(&closes(klines), period)
        .context("indicator rsi_at: TA-Lib RSI calculation failed")?;
    result.get(idx).copied().ok_or_else(|| {
        VirsError::config(format!(
            "indicator rsi_at: insufficient data at idx={idx} (no result produced, period={period})"
        ))
    })
}

#[inline(always)]
pub fn macd_at(klines: &[Kline], idx: usize, fast: usize, slow: usize) -> VirsResult<f64> {
    if klines.is_empty() || idx < slow - 1 {
        return Err(VirsError::config(format!(
            "indicator macd_at: insufficient data at idx={idx} (fast={fast}, slow={slow})"
        )));
    }
    let (macd, _, _) = momentum::macd(&closes(klines), fast, slow, 9)
        .context("indicator macd_at: TA-Lib MACD calculation failed")?;
    macd.get(idx).copied().ok_or_else(|| {
        VirsError::config(format!(
            "indicator macd_at: insufficient data at idx={idx} (no result produced, fast={fast}, slow={slow})"
        ))
    })
}

#[inline(always)]
pub fn macd_signal_at(
    klines: &[Kline],
    idx: usize,
    fast: usize,
    slow: usize,
    signal: usize,
) -> VirsResult<f64> {
    if klines.is_empty() || idx < slow + signal - 2 {
        return Err(VirsError::config(format!(
            "indicator macd_signal_at: insufficient data at idx={idx} (fast={fast}, slow={slow}, signal={signal})"
        )));
    }
    let (_, sig, _) = momentum::macd(&closes(klines), fast, slow, signal)
        .context("indicator macd_signal_at: TA-Lib MACD calculation failed")?;
    sig.get(idx).copied().ok_or_else(|| {
        VirsError::config(format!(
            "indicator macd_signal_at: insufficient data at idx={idx} (no result produced, fast={fast}, slow={slow}, signal={signal})"
        ))
    })
}

#[inline(always)]
pub fn bbands_at(
    klines: &[Kline],
    idx: usize,
    period: usize,
    std_dev: f64,
) -> VirsResult<(f64, f64, f64)> {
    if klines.is_empty() || idx < period - 1 {
        return Err(VirsError::config(format!(
            "indicator bbands_at: insufficient data at idx={idx} (period={period}, std_dev={std_dev})"
        )));
    }
    let (upper, middle, lower) =
        overlap::bbands(&closes(klines), period, std_dev, std_dev, MaType::Sma)
            .context("indicator bbands_at: TA-Lib BBands calculation failed")?;
    let upper = upper.get(idx).copied().ok_or_else(|| {
        VirsError::config(format!(
            "indicator bbands_at.upper: insufficient data at idx={idx} (no result produced, period={period})"
        ))
    })?;
    let middle = middle.get(idx).copied().ok_or_else(|| {
        VirsError::config(format!(
            "indicator bbands_at.middle: insufficient data at idx={idx} (no result produced, period={period})"
        ))
    })?;
    let lower = lower.get(idx).copied().ok_or_else(|| {
        VirsError::config(format!(
            "indicator bbands_at.lower: insufficient data at idx={idx} (no result produced, period={period})"
        ))
    })?;
    Ok((upper, middle, lower))
}

#[inline(always)]
pub fn atr_at(klines: &[Kline], idx: usize, period: usize) -> VirsResult<f64> {
    if klines.is_empty() || idx < period {
        return Err(VirsError::config(format!(
            "indicator atr_at: insufficient data at idx={idx} (period={period})"
        )));
    }
    let result =
        volatility::atr(&highs(klines), &lows(klines), &closes(klines), period)
            .context("indicator atr_at: TA-Lib ATR calculation failed")?;
    result.get(idx).copied().ok_or_else(|| {
        VirsError::config(format!(
            "indicator atr_at: insufficient data at idx={idx} (no result produced, period={period})"
        ))
    })
}

#[inline(always)]
pub fn adx_at(klines: &[Kline], idx: usize, period: usize) -> VirsResult<f64> {
    if klines.is_empty() || idx < period * 2 {
        return Err(VirsError::config(format!(
            "indicator adx_at: insufficient data at idx={idx} (period={period})"
        )));
    }
    let result =
        momentum::adx(&highs(klines), &lows(klines), &closes(klines), period)
            .context("indicator adx_at: TA-Lib ADX calculation failed")?;
    result.get(idx).copied().ok_or_else(|| {
        VirsError::config(format!(
            "indicator adx_at: insufficient data at idx={idx} (no result produced, period={period})"
        ))
    })
}

#[inline(always)]
pub fn macd_histogram_at(
    klines: &[Kline],
    idx: usize,
    fast: usize,
    slow: usize,
    signal: usize,
) -> VirsResult<f64> {
    if klines.is_empty() || idx < slow + signal - 2 {
        return Err(VirsError::config(format!(
            "indicator macd_histogram_at: insufficient data at idx={idx} (fast={fast}, slow={slow}, signal={signal})"
        )));
    }
    let (macd, sig, _) = momentum::macd(&closes(klines), fast, slow, signal)
        .context("indicator macd_histogram_at: TA-Lib MACD calculation failed")?;
    let m = macd.get(idx).copied().ok_or_else(|| {
        VirsError::config(format!(
            "indicator macd_histogram_at.macd: insufficient data at idx={idx} (no result produced, fast={fast}, slow={slow}, signal={signal})"
        ))
    })?;
    let s = sig.get(idx).copied().ok_or_else(|| {
        VirsError::config(format!(
            "indicator macd_histogram_at.signal: insufficient data at idx={idx} (no result produced, fast={fast}, slow={slow}, signal={signal})"
        ))
    })?;
    Ok(m - s)
}

#[inline(always)]
pub fn bbands_width_at(klines: &[Kline], idx: usize, period: usize, std_dev: f64) -> VirsResult<f64> {
    let (upper, middle, lower) = bbands_at(klines, idx, period, std_dev)?;
    if middle == 0.0 {
        return Err(VirsError::config(format!(
            "indicator bbands_width_at: middle band is 0.0 at idx={idx} (period={period}, std_dev={std_dev}) — cannot divide by zero"
        )));
    }
    Ok((upper - lower) / middle)
}

#[inline(always)]
pub fn highest_at(klines: &[Kline], idx: usize, period: usize) -> VirsResult<f64> {
    if klines.is_empty() || idx < period - 1 || period == 0 {
        return Err(VirsError::config(format!(
            "indicator highest_at: insufficient data at idx={idx} (period={period})"
        )));
    }
    let result = math_operator::max(&highs(klines), period)
        .context("indicator highest_at: TA-Lib MAX calculation failed")?;
    result.get(idx).copied().ok_or_else(|| {
        VirsError::config(format!(
            "indicator highest_at: insufficient data at idx={idx} (no result produced, period={period})"
        ))
    })
}

#[inline(always)]
pub fn lowest_at(klines: &[Kline], idx: usize, period: usize) -> VirsResult<f64> {
    if klines.is_empty() || idx < period - 1 || period == 0 {
        return Err(VirsError::config(format!(
            "indicator lowest_at: insufficient data at idx={idx} (period={period})"
        )));
    }
    let result = math_operator::min(&lows(klines), period)
        .context("indicator lowest_at: TA-Lib MIN calculation failed")?;
    result.get(idx).copied().ok_or_else(|| {
        VirsError::config(format!(
            "indicator lowest_at: insufficient data at idx={idx} (no result produced, period={period})"
        ))
    })
}

pub fn volume_sma_at(klines: &[Kline], idx: usize, period: usize) -> VirsResult<f64> {
    if idx < period - 1 || klines.is_empty() {
        return Err(VirsError::config(format!(
            "indicator volume_sma_at: insufficient data at idx={idx} (period={period})"
        )));
    }
    let start = idx + 1 - period;
    let sum: f64 = (start..=idx).map(|i| klines[i].volume).sum();
    Ok(sum / period as f64)
}

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

pub fn find_round_number(price: f64, upward: bool) -> f64 {
    if price <= 0.0 {
        return 0.0;
    }
    let magnitude = 10_f64.powf(price.log10().floor());
    let step = if magnitude >= 10000.0 {
        1000.0
    } else if magnitude >= 1000.0 {
        100.0
    } else if magnitude >= 100.0 {
        10.0
    } else if magnitude >= 10.0 {
        5.0
    } else {
        1.0
    };
    if upward {
        (price / step).ceil() * step
    } else {
        (price / step).floor() * step
    }
}

pub fn compute_ema_cross_bars_ago(
    klines: &[Kline],
    fast_period: usize,
    slow_period: usize,
    last_idx: usize,
) -> VirsResult<i32> {
    if klines.len() < slow_period + 5 {
        return Err(VirsError::config(format!(
            "indicator compute_ema_cross_bars_ago: insufficient data (klines={}, need {})",
            klines.len(),
            slow_period + 5
        )));
    }
    let lookback = 20.min(last_idx);
    for i in 0..lookback {
        let idx = last_idx - i;
        if idx < 1 {
            break;
        }
        let fast_curr = ema_at(klines, idx, fast_period)?;
        let slow_curr = ema_at(klines, idx, slow_period)?;
        let fast_prev = ema_at(klines, idx - 1, fast_period)?;
        let slow_prev = ema_at(klines, idx - 1, slow_period)?;
        if (fast_prev <= slow_prev && fast_curr > slow_curr)
            || (fast_prev >= slow_prev && fast_curr < slow_curr)
        {
            return Ok(i as i32);
        }
    }
    Ok(-1)
}
