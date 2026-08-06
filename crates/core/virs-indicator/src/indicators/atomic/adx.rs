//! ADX 指标（TA-Lib `momentum::adx`）。

use talib_rs::momentum;
use virs_error::{Context, VirsError, VirsResult};
use virs_type::Kline;

use crate::indicators::{closes, highs, lows};

/// 计算指定索引处的 ADX 值。
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
