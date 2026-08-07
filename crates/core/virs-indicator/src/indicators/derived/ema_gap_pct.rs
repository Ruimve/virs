

use virs_error::{VirsError, VirsResult};
use virs_type::Kline;

use crate::indicators::atomic::ema::ema_at;


/* EMA 间距百分比：(快线 - 慢线) / 慢线 * 100，衡量快慢线偏离程度 */
pub fn compute(klines: &[Kline], fast: usize, slow: usize) -> VirsResult<f64> {
    let last_idx = klines.len().saturating_sub(1);
    if last_idx < slow - 1 {
        return Err(VirsError::config(format!(
            "EmaGapPct: insufficient data (last_idx={last_idx}, slow={slow})"
        )));
    }
    let ema_fast = ema_at(klines, last_idx, fast)?;
    let ema_slow = ema_at(klines, last_idx, slow)?;
    if ema_slow == 0.0 {
        return Err(VirsError::config(format!(
            "EmaGapPct: ema_slow is 0.0 (slow={slow}) — cannot divide by zero"
        )));
    }
    Ok((ema_fast - ema_slow) / ema_slow * 100.0)
}
