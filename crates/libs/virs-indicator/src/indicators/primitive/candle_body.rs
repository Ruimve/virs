//! K 线实体（close - open）。

use virs_error::{VirsError, VirsResult};
use virs_types::Kline;

/// 计算最新 K 线的实体大小。
pub fn compute(klines: &[Kline]) -> VirsResult<f64> {
    let last = klines.last().ok_or_else(|| {
        VirsError::config("CandleBody: klines is empty — cannot get candle body")
    })?;
    Ok(last.close - last.open)
}
