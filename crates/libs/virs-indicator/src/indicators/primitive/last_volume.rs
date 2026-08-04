//! 已完成 K 线的成交量。

use virs_error::{VirsError, VirsResult};
use virs_types::Kline;

/// 获取最后一根已完成 K 线的成交量。
pub fn compute(klines: &[Kline]) -> VirsResult<f64> {
    let last_completed = klines.len().saturating_sub(2);
    klines.get(last_completed).map(|k| k.volume).ok_or_else(|| {
        VirsError::config(format!(
            "LastCompletedVolume: insufficient data (klines_len={}, need >= 2)",
            klines.len()
        ))
    })
}
