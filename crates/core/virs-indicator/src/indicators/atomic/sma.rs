

use talib_rs::overlap;
use virs_error::{Context, VirsError, VirsResult};


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
