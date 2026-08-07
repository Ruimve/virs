

use talib_rs::momentum;
use virs_error::{Context, VirsError, VirsResult};
use virs_type::Kline;

use crate::indicators::{closes, highs, lows};


/* ADX 指标计算：平均趋向指数，衡量趋势强度（不区分方向），需要 period*2 的数据量 */
pub fn adx_at(klines: &[Kline], idx: usize, period: usize) -> VirsResult<f64> {
    if klines.is_empty() || idx < period * 2 {
        return Err(VirsError::config(format!(
            "indicator adx_at: insufficient data at idx={idx} (period={period})"
        )));
    }
    let result =
        momentum::adx(&highs(klines), &lows(klines), &closes(klines), period)
            .context("indicator adx_at: TA-Lib ADX calculation failed")?;
    result.get(idx).copied().ok_or_else(|| {
        VirsError::config(format!(
            "indicator adx_at: insufficient data at idx={idx} (no result produced, period={period})"
        ))
    })
}
