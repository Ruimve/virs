

use talib_rs::momentum;
use virs_error::{Context, VirsError, VirsResult};
use virs_type::Kline;

use crate::indicators::closes;


pub fn macd_at(klines: &[Kline], idx: usize, fast: usize, slow: usize) -> VirsResult<f64> {
    if klines.is_empty() || idx < slow - 1 {
        return Err(VirsError::config(format!(
            "indicator macd_at: insufficient data at idx={idx} (fast={fast}, slow={slow})"
        )));
    }
    let (macd, _, _) = momentum::macd(&closes(klines), fast, slow, 9)
        .context("indicator macd_at: TA-Lib MACD calculation failed")?;
    macd.get(idx).copied().ok_or_else(|| {
        VirsError::config(format!(
            "indicator macd_at: insufficient data at idx={idx} (no result produced, fast={fast}, slow={slow})"
        ))
    })
}


pub fn macd_signal_at(
    klines: &[Kline],
    idx: usize,
    fast: usize,
    slow: usize,
    signal: usize,
) -> VirsResult<f64> {
    if klines.is_empty() || idx < slow + signal - 2 {
        return Err(VirsError::config(format!(
            "indicator macd_signal_at: insufficient data at idx={idx} (fast={fast}, slow={slow}, signal={signal})"
        )));
    }
    let (_, sig, _) = momentum::macd(&closes(klines), fast, slow, signal)
        .context("indicator macd_signal_at: TA-Lib MACD calculation failed")?;
    sig.get(idx).copied().ok_or_else(|| {
        VirsError::config(format!(
            "indicator macd_signal_at: insufficient data at idx={idx} (no result produced, fast={fast}, slow={slow}, signal={signal})"
        ))
    })
}
