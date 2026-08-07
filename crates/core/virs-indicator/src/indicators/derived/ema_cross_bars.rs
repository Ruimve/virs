

use virs_error::{VirsError, VirsResult};
use virs_type::Kline;

use crate::indicators::atomic::ema::ema_at;


/* EMA 交叉距今 K 线数：从最新 K 线向前搜索最近一次金叉或死叉，返回距今的 K 线根数，无交叉返回 -1 */
pub fn compute(klines: &[Kline], fast: usize, slow: usize) -> VirsResult<i32> {
    let last_idx = klines.len().saturating_sub(1);
    if klines.len() < slow + 5 {
        return Err(VirsError::config(format!(
            "EmaCrossBarsAgo: insufficient data (klines={}, need >= {})",
            klines.len(),
            slow + 5
        )));
    }
    /* 最多向前搜索 20 根 K 线 */
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
        /* 交叉判定：前一根快线在慢线下方/上方，当前根穿越到另一侧 */
        if (fast_prev <= slow_prev && fast_curr > slow_curr)
            || (fast_prev >= slow_prev && fast_curr < slow_curr)
        {
            return Ok(i as i32);
        }
    }
    /* 20 根 K 线内无交叉 */
    Ok(-1)
}
