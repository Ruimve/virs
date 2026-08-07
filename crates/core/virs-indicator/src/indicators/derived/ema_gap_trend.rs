

use virs_error::{VirsError, VirsResult};
use virs_type::Kline;

use crate::indicators::atomic::ema::ema_at;


/* EMA 间距趋势：比较当前与 5 根 K 线前的快慢线间距，判断间距在扩大/缩小/持平 */
pub fn compute(klines: &[Kline], fast: usize, slow: usize) -> VirsResult<String> {
    let last_idx = klines.len().saturating_sub(1);
    if last_idx < slow - 1 {
        return Err(VirsError::config(format!(
            "EmaGapTrend: insufficient data (last_idx={last_idx}, slow={slow})"
        )));
    }
    let ema_fast = ema_at(klines, last_idx, fast)?;
    let ema_slow = ema_at(klines, last_idx, slow)?;
    /* 回看 5 根 K 线的快慢线间距作为对比基准 */
    let lookback = 5.min(last_idx);
    let ema_fast_prev = ema_at(klines, last_idx - lookback, fast)?;
    let ema_slow_prev = if klines.len() >= slow + lookback {
        ema_at(klines, last_idx - lookback, slow)?
    } else {
        ema_slow
    };
    let curr_gap_abs = (ema_fast - ema_slow).abs();
    let prev_gap_abs = (ema_fast_prev - ema_slow_prev).abs();
    /* 间距变化超过 1% 阈值才判定为扩大或缩小，避免微小波动误判 */
    let trend = if curr_gap_abs > prev_gap_abs * 1.01 {
        "扩大"
    } else if curr_gap_abs < prev_gap_abs * 0.99 {
        "缩小"
    } else {
        "持平"
    };
    Ok(trend.to_string())
}
