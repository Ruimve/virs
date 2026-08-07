

use virs_error::{VirsError, VirsResult};
use virs_type::Kline;

use crate::indicators::atomic::bbands::bbands_at;


/* 布林带宽度：(上轨 - 下轨) / 中轨，衡量价格波动收缩/扩张程度 */
pub fn compute(klines: &[Kline], idx: usize, period: usize, stddev: u32) -> VirsResult<f64> {
    let (upper, middle, lower) = bbands_at(klines, idx, period, stddev as f64)?;
    if middle == 0.0 {
        return Err(VirsError::config(format!(
            "BbandsWidth: middle band is 0.0 at idx={idx} (period={period}, stddev={stddev}) — cannot divide by zero"
        )));
    }
    Ok((upper - lower) / middle)
}
