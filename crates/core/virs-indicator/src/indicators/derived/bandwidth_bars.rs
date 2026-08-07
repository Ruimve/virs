

use virs_error::{VirsError, VirsResult};
use virs_type::Kline;

use crate::indicators::derived::bbands_width;


/* N 根 K 线前的布林带宽度：回退 bars_ago 根 K 线计算带宽，用于对比当前与历史波动率 */
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
