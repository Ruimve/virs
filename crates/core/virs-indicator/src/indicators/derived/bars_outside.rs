

use virs_error::{VirsError, VirsResult};
use virs_type::Kline;

use crate::indicators::atomic::bbands::bbands_at;


/* 布林带外 K 线计数：从最新 K 线向前统计连续在布林带上方(+1)或下方(-1)的 K 线数，回到带内则停止 */
pub fn compute(klines: &[Kline], period: usize, stddev: u32) -> VirsResult<i32> {
    let last_idx = klines.len().saturating_sub(1);
    if last_idx < period - 1 {
        return Err(VirsError::config(format!(
            "BarsOutsideBand: insufficient data (last_idx={last_idx}, period={period})"
        )));
    }
    let (upper, _, lower) = bbands_at(klines, last_idx, period, stddev as f64)?;
    let mut count: i32 = 0;
    /* 从最新 K 线倒序遍历：收盘价在上轨之上 +1，在下轨之下 -1，在带内则停止 */
    for k in klines.iter().rev() {
        if k.close > upper {
            count += 1;
        } else if k.close < lower {
            count -= 1;
        } else {
            break;
        }
    }
    Ok(count)
}
