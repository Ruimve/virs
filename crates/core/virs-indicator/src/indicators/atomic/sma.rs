

use talib_rs::overlap;
use virs_error::{Context, VirsError, VirsResult};


/* SMA 计算（支持 NaN 过滤）：对任意数值序列做简单移动平均，过滤 NaN 值后计算 */
pub fn sma_at_from(series: &[f64], idx: usize, period: usize) -> VirsResult<f64> {
    if series.is_empty() || period == 0 {
        return Err(VirsError::config(format!(
            "indicator sma_at_from: insufficient data at idx={idx} (empty series or period=0, period={period})"
        )));
    }
    /* 统计 idx 及之前的 NaN 数量，用于将原始索引映射到过滤后的序列索引 */
    let nan_count = series.iter().take(idx + 1).filter(|v| v.is_nan()).count();
    /* 过滤 NaN 值，TA-Lib 的 SMA 不接受 NaN */
    let valid: Vec<f64> = series.iter().filter(|v| !v.is_nan()).copied().collect();
    if valid.len() < period {
        /* 有效数据不足 period 时，用所有有效值的平均作为退化结果 */
        if valid.is_empty() {
            return Err(VirsError::config(format!(
                "indicator sma_at_from: insufficient data at idx={idx} (no valid values, period={period})"
            )));
        }
        return Ok(valid
            .iter()
            .rev()
            .take(period.min(valid.len()))
            .sum::<f64>()
            / period.min(valid.len()) as f64);
    }
    /* 将原始索引减去 NaN 数量得到过滤后序列中的对应索引 */
    let mapped_idx = idx.saturating_sub(nan_count);
    let result = overlap::sma(&valid, period)
        .context("indicator sma_at_from: TA-Lib SMA calculation failed")?;
    result
        .get(mapped_idx)
        .copied()
        .or_else(|| result.last().copied())
        .ok_or_else(|| {
            VirsError::config(format!(
                "indicator sma_at_from: insufficient data at idx={idx} (no result produced, period={period})"
            ))
        })
}
