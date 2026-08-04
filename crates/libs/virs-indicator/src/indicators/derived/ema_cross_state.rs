//! EMA 交叉状态（"金叉(多头)" / "死叉(空头)"）。
//!
//! 依赖 [`crate::indicators::atomic::ema`]。

use virs_error::{VirsError, VirsResult};
use virs_types::Kline;

use crate::indicators::atomic::ema::ema_at;

/// 计算最新 K 线的 EMA 交叉状态。
///
/// `ema_fast > ema_slow` 为金叉(多头)，否则为死叉(空头)。
pub fn compute(klines: &[Kline], fast: usize, slow: usize) -> VirsResult<String> {
    let last_idx = klines.len().saturating_sub(1);
    if last_idx < slow - 1 {
        return Err(VirsError::config(format!(
            "EmaCrossState: insufficient data (last_idx={last_idx}, slow={slow})"
        )));
    }
    let ema_fast = ema_at(klines, last_idx, fast)?;
    let ema_slow = ema_at(klines, last_idx, slow)?;
    Ok(if ema_fast > ema_slow {
        "金叉(多头)".to_string()
    } else {
        "死叉(空头)".to_string()
    })
}
