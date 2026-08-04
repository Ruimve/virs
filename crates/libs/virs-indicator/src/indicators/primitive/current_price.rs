//! 当前价格（最新 K 线收盘价）。

use virs_error::{VirsError, VirsResult};
use virs_type::Kline;

/// 获取最新 K 线的收盘价。
pub fn compute(klines: &[Kline]) -> VirsResult<f64> {
    klines.last().map(|k| k.close).ok_or_else(|| {
        VirsError::config("CurrentPrice: klines is empty — cannot get current price")
    })
}
