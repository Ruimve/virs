//! N 根 K 线前的布林带宽度。
//!
//! 依赖 [`crate::indicators::derived::bbands_width`]。

use virs_error::{VirsError, VirsResult};
use virs_type::Kline;

use crate::indicators::derived::bbands_width;

/// 计算指定索引处 N 根 K 线前的布林带宽度比。
pub fn compute(
    klines: &[Kline],
    period: usize,
    stddev: u32,
    bars_ago: usize,
) -> VirsResult<f64> {
    let last_idx = klines.len().saturating_sub(1);
    if last_idx < bars_ago || last_idx - bars_ago < period - 1 {
        return Err(VirsError::config(format!(
            "BandwidthBarsAgo: insufficient data (last_idx={last_idx}, period={period}, bars_ago={bars_ago})"
        )));
    }
    bbands_width::compute(klines, last_idx - bars_ago, period, stddev)
}
