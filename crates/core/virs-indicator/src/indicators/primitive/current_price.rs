

use virs_error::{VirsError, VirsResult};
use virs_type::Kline;


pub fn compute(klines: &[Kline]) -> VirsResult<f64> {
    klines.last().map(|k| k.close).ok_or_else(|| {
        VirsError::config("CurrentPrice: klines is empty — cannot get current price")
    })
}
