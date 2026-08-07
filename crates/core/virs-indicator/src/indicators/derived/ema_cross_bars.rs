

use virs_error::{VirsError, VirsResult};
use virs_type::Kline;

use crate::indicators::atomic::ema::ema_at;


pub fn compute(klines: &[Kline], fast: usize, slow: usize) -> VirsResult<i32> {
    let last_idx = klines.len().saturating_sub(1);
    if klines.len() < slow + 5 {
        return Err(VirsError::config(format!(
            "EmaCrossBarsAgo: insufficient data (klines={}, need >= {})",
            klines.len(),
            slow + 5
        )));
    }
    let lookback = 20.min(last_idx);
    for i in 0..lookback {
        let idx = last_idx - i;
        if idx < 1 {
            break;
        }
        let fast_curr = ema_at(klines, idx, fast)?;
        let slow_curr = ema_at(klines, idx, slow)?;
        let fast_prev = ema_at(klines, idx - 1, fast)?;
        let slow_prev = ema_at(klines, idx - 1, slow)?;
        if (fast_prev <= slow_prev && fast_curr > slow_curr)
            || (fast_prev >= slow_prev && fast_curr < slow_curr)
        {
            return Ok(i as i32);
        }
    }
    Ok(-1)
}
