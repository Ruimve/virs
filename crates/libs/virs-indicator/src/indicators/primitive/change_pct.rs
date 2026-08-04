//! 涨跌幅（N 根 K 线前收盘价到当前的百分比变化）。

use virs_error::{VirsError, VirsResult};
use virs_type::Kline;

/// 计算最新 K 线相对 N 根前的涨跌幅（百分比）。
pub fn compute(klines: &[Kline], period: usize) -> VirsResult<f64> {
    let last_idx = klines.len().saturating_sub(1);
    if last_idx < period || period == 0 {
        return Err(VirsError::config(format!(
            "ChangePct: insufficient data (last_idx={last_idx}, period={period})"
        )));
    }
    let curr = klines[last_idx].close;
    let prev = klines[last_idx - period].close;
    if prev == 0.0 {
        return Err(VirsError::config(format!(
            "ChangePct: previous close price is 0.0 (idx={}) — cannot compute percentage change with zero base",
            last_idx - period
        )));
    }
    Ok((curr - prev) / prev * 100.0)
}
