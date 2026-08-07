

use virs_error::{VirsError, VirsResult};
use virs_type::Kline;

use crate::indicators::atomic::atr::atr_at;


/* ATR 百分比：ATR / 当前价格 * 100，用于标准化不同价格级别的波动率比较 */
pub fn compute(klines: &[Kline], period: usize) -> VirsResult<f64> {
    let last_idx = klines.len().saturating_sub(1);
    if last_idx < period {
        return Err(VirsError::config(format!(
            "AtrPct: insufficient data (last_idx={last_idx}, period={period})"
        )));
    }
    let atr_val = atr_at(klines, last_idx, period)?;
    let price = klines
        .last()
        .map(|k| k.close)
        .ok_or_else(|| VirsError::config("AtrPct: klines is empty"))?;
    if price <= 0.0 {
        return Err(VirsError::config(format!(
            "AtrPct: current price is {price} — cannot compute ATR percentage"
        )));
    }
    Ok(atr_val / price * 100.0)
}
