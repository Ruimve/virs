//! 布林带宽度比 ((upper - lower) / middle)。
//!
//! 依赖 [`crate::indicators::atomic::bbands`]。

use virs_error::{VirsError, VirsResult};
use virs_types::Kline;

use crate::indicators::atomic::bbands::bbands_at;

/// 计算指定索引处的布林带宽度比。
pub fn compute(klines: &[Kline], idx: usize, period: usize, stddev: u32) -> VirsResult<f64> {
    let (upper, middle, lower) = bbands_at(klines, idx, period, stddev as f64)?;
    if middle == 0.0 {
        return Err(VirsError::config(format!(
            "BbandsWidth: middle band is 0.0 at idx={idx} (period={period}, stddev={stddev}) — cannot divide by zero"
        )));
    }
    Ok((upper - lower) / middle)
}
