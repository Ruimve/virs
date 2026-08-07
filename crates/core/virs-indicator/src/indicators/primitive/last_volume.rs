

use virs_error::{VirsError, VirsResult};
use virs_type::Kline;


/* 最近已完成 K 线的成交量：取倒数第二根（最后一根可能尚未收盘） */
pub fn compute(klines: &[Kline]) -> VirsResult<f64> {
    if klines.len() < 2 {
        return Err(VirsError::config(format!(
            "LastCompletedVolume: insufficient data (klines_len={}, need >= 2)",
            klines.len()
        )));
    }
    /* 最后一根 K 线可能正在形成中，取倒数第二根作为已完成的成交量 */
    let last_completed = klines.len() - 2;
    Ok(klines[last_completed].volume)
}
