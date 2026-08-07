

use virs_error::{VirsError, VirsResult};
use virs_type::Kline;

use crate::indicators::derived::bbands_width;


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
