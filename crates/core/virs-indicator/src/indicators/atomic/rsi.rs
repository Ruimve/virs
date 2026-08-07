

use talib_rs::momentum;
use virs_error::{Context, VirsError, VirsResult};
use virs_type::Kline;

use crate::indicators::closes;


pub fn rsi_at(klines: &[Kline], idx: usize, period: usize) -> VirsResult<f64> {
    if klines.is_empty() || idx < period || period == 0 {
        return Err(VirsError::config(format!(
            "indicator rsi_at: insufficient data at idx={idx} (period={period})"
        )));
    }
    let result = momentum::rsi(&closes(klines), period)
        .context("indicator rsi_at: TA-Lib RSI calculation failed")?;
    result.get(idx).copied().ok_or_else(|| {
        VirsError::config(format!(
            "indicator rsi_at: insufficient data at idx={idx} (no result produced, period={period})"
        ))
    })
}
