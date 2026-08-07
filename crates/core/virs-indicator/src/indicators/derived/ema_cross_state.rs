

use virs_error::{VirsError, VirsResult};
use virs_type::Kline;

use crate::indicators::atomic::ema::ema_at;


/* EMA 交叉状态：快线在慢线上方为金叉（多头），反之为死叉（空头） */
pub fn compute(klines: &[Kline], fast: usize, slow: usize) -> VirsResult<String> {
    let last_idx = klines.len().saturating_sub(1);
    if last_idx < slow - 1 {
        return Err(VirsError::config(format!(
            "EmaCrossState: insufficient data (last_idx={last_idx}, slow={slow})"
        )));
    }
    let ema_fast = ema_at(klines, last_idx, fast)?;
    let ema_slow = ema_at(klines, last_idx, slow)?;
    /* 快线 > 慢线为金叉（多头趋势），反之为死叉（空头趋势） */
    Ok(if ema_fast > ema_slow {
        "金叉(多头)".to_string()
    } else {
        "死叉(空头)".to_string()
    })
}
