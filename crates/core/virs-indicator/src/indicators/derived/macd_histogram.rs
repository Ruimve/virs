

use virs_error::{VirsError, VirsResult};
use virs_type::Kline;

use crate::indicators::atomic::macd::{macd_at, macd_signal_at};


pub fn compute(klines: &[Kline], fast: usize, slow: usize, signal: usize) -> VirsResult<f64> {
    let last_idx = klines.len().saturating_sub(1);
    if last_idx < slow + signal - 2 {
        return Err(VirsError::config(format!(
            "MacdHistogram: insufficient data (last_idx={last_idx}, fast={fast}, slow={slow}, signal={signal})"
        )));
    }
    let m = macd_at(klines, last_idx, fast, slow)?;
    let s = macd_signal_at(klines, last_idx, fast, slow, signal)?;
    Ok(m - s)
}
