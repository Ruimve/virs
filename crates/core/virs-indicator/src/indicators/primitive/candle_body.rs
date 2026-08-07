

use virs_error::{VirsError, VirsResult};
use virs_type::Kline;


pub fn compute(klines: &[Kline]) -> VirsResult<f64> {
    let last = klines.last().ok_or_else(|| {
        VirsError::config("CandleBody: klines is empty — cannot get candle body")
    })?;
    Ok(last.close - last.open)
}
