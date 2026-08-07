

use virs_error::{VirsError, VirsResult};
use virs_type::Kline;

use crate::indicators::atomic::atr::atr;
use crate::indicators::atomic::sma::sma_at_from;


/* ATR 的 SMA：先计算 ATR 序列，再对其做简单移动平均，用于平滑波动率 */
pub fn compute(klines: &[Kline], atr_period: usize, sma_period: usize) -> VirsResult<f64> {
    if klines.len() < sma_period {
        return Err(VirsError::config(format!(
            "AtrSma: insufficient data (klines={}, need >= {sma_period})",
            klines.len()
        )));
    }
    let atr_series = atr(klines, atr_period)?;
    let last_idx = klines.len().saturating_sub(1);
    sma_at_from(&atr_series, last_idx, sma_period)
}
