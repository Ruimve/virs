

use talib_rs::volatility;
use virs_error::{Context, VirsError, VirsResult};
use virs_type::Kline;

use crate::indicators::{closes, highs, lows};


pub fn atr(klines: &[Kline], period: usize) -> VirsResult<Vec<f64>> {
    Ok(volatility::atr(&highs(klines), &lows(klines), &closes(klines), period)
        .context("indicator atr: TA-Lib ATR calculation failed")?)
}


pub fn atr_at(klines: &[Kline], idx: usize, period: usize) -> VirsResult<f64> {
    if klines.is_empty() || idx < period {
        return Err(VirsError::config(format!(
            "indicator atr_at: insufficient data at idx={idx} (period={period})"
        )));
    }
    let result =
        volatility::atr(&highs(klines), &lows(klines), &closes(klines), period)
            .context("indicator atr_at: TA-Lib ATR calculation failed")?;
    result.get(idx).copied().ok_or_else(|| {
        VirsError::config(format!(
            "indicator atr_at: insufficient data at idx={idx} (no result produced, period={period})"
        ))
    })
}
