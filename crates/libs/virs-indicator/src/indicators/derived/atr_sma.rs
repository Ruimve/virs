//! ATR 序列的 SMA。
//!
//! 依赖 [`crate::indicators::atomic::atr`] + [`crate::indicators::atomic::sma`]。

use virs_error::{VirsError, VirsResult};
use virs_types::Kline;

use crate::indicators::atomic::atr::atr;
use crate::indicators::atomic::sma::sma_at_from;

/// 计算最新 K 线的 ATR SMA 值。
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
