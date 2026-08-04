//! 成交量 SMA 指标（手写，非 TA-Lib）。

use virs_error::{VirsError, VirsResult};
use virs_type::Kline;

/// 计算指定索引处过去 N 根 K 线的成交量 SMA。
pub fn volume_sma_at(klines: &[Kline], idx: usize, period: usize) -> VirsResult<f64> {
    if idx < period - 1 || klines.is_empty() {
        return Err(VirsError::config(format!(
            "indicator volume_sma_at: insufficient data at idx={idx} (period={period})"
        )));
    }
    let start = idx + 1 - period;
    let sum: f64 = (start..=idx).map(|i| klines[i].volume).sum();
    Ok(sum / period as f64)
}
