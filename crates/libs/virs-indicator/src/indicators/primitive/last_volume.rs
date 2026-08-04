//! 已完成 K 线的成交量。

use virs_error::{VirsError, VirsResult};
use virs_types::Kline;

/// 获取最后一根已完成 K 线的成交量。
pub fn compute(klines: &[Kline]) -> VirsResult<f64> {
    if klines.len() < 2 {
        return Err(VirsError::config(format!(
            "LastCompletedVolume: insufficient data (klines_len={}, need >= 2)",
            klines.len()
        )));
    }
    let last_completed = klines.len() - 2;
    Ok(klines[last_completed].volume)
}
