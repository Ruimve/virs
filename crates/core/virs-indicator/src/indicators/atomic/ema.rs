//! EMA 指标（TA-Lib `overlap::ema`）。

use talib_rs::overlap;
use virs_error::{Context, VirsError, VirsResult};
use virs_type::Kline;

use crate::indicators::closes;

/// 计算指定索引处的 EMA 值。
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
