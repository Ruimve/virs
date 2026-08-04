//! 最高价指标（TA-Lib `math_operator::max`）。

use talib_rs::math_operator;
use virs_error::{Context, VirsError, VirsResult};
use virs_types::Kline;

use crate::indicators::highs;

/// 计算指定索引处过去 N 根 K 线的最高价。
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
