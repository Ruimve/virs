

use talib_rs::math_operator;
use virs_error::{Context, VirsError, VirsResult};
use virs_type::Kline;

use crate::indicators::lows;


pub fn lowest_at(klines: &[Kline], idx: usize, period: usize) -> VirsResult<f64> {
    if klines.is_empty() || idx < period - 1 || period == 0 {
        return Err(VirsError::config(format!(
            "indicator lowest_at: insufficient data at idx={idx} (period={period})"
        )));
    }
    let result = math_operator::min(&lows(klines), period)
        .context("indicator lowest_at: TA-Lib MIN calculation failed")?;
    result.get(idx).copied().ok_or_else(|| {
        VirsError::config(format!(
            "indicator lowest_at: insufficient data at idx={idx} (no result produced, period={period})"
        ))
    })
}
