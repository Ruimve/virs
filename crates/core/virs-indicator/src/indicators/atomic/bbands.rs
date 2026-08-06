//! 布林带指标（TA-Lib `overlap::bbands`）。

use talib_rs::{ma_type::MaType, overlap};
use virs_error::{Context, VirsError, VirsResult};
use virs_type::Kline;

use crate::indicators::closes;

/// 计算指定索引处的布林带三线值 (upper, middle, lower)。
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
