//! 派生指标（Layer 1+2）。
//!
//! 组合一个或多个 [`crate::indicators::atomic`] 函数进行二次计算。
//! 每个文件导出 `compute(klines, params) -> VirsResult<T>` 函数。

pub mod atr_pct;
pub mod atr_sma;
pub mod bars_outside;
pub mod bandwidth_bars;
pub mod bbands_width;
pub mod ema_cross_bars;
pub mod ema_cross_state;
pub mod ema_gap_pct;
pub mod ema_gap_trend;
pub mod macd_histogram;
pub mod round_number;
