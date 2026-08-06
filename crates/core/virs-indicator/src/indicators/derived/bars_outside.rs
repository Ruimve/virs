//! 连续出轨 K 线数（正=超上轨，负=破下轨）。
//!
//! 依赖 [`crate::indicators::atomic::bbands`]。

use virs_error::{VirsError, VirsResult};
use virs_type::Kline;

use crate::indicators::atomic::bbands::bbands_at;

/// 计算最新 K 线的连续出轨根数。
///
/// 正数表示连续收盘价超上轨，负数表示破下轨，0 表示未出轨。
pub fn compute(klines: &[Kline], period: usize, stddev: u32) -> VirsResult<i32> {
    let last_idx = klines.len().saturating_sub(1);
    if last_idx < period - 1 {
        return Err(VirsError::config(format!(
            "BarsOutsideBand: insufficient data (last_idx={last_idx}, period={period})"
        )));
    }
    let (upper, _, lower) = bbands_at(klines, last_idx, period, stddev as f64)?;
    let mut count: i32 = 0;
    for k in klines.iter().rev() {
        if k.close > upper {
            count += 1;
        } else if k.close < lower {
            count -= 1;
        } else {
            break;
        }
    }
    Ok(count)
}
