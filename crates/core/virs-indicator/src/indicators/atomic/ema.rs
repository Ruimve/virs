

use talib_rs::overlap;
use virs_error::{Context, VirsError, VirsResult};
use virs_type::Kline;

use crate::indicators::closes;


/* EMA 指标计算：调用 TA-Lib 计算指数移动平均线，返回指定索引处的值 */
pub fn ema_at(klines: &[Kline], idx: usize, period: usize) -> VirsResult<f64> {
    if klines.is_empty() || idx < period - 1 || period == 0 {
        return Err(VirsError::config(format!(
            "indicator ema_at: insufficient data at idx={idx} (period={period})"
        )));
    }
    let result = overlap::ema(&closes(klines), period)
        .context("indicator ema_at: TA-Lib EMA calculation failed")?;
    result.get(idx).copied().ok_or_else(|| {
        VirsError::config(format!(
            "indicator ema_at: insufficient data at idx={idx} (no result produced, period={period})"
        ))
    })
}
