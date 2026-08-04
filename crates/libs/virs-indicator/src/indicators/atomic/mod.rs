//! TA-Lib 直接封装（Layer 0）。
//!
//! 每个文件封装一个 TA-Lib 函数族，返回 `VirsResult<f64>` 或 `Vec<f64>`。
//! 数据不足时返回 `Err`，不使用默认值。

pub mod adx;
pub mod atr;
pub mod bbands;
pub mod ema;
pub mod highest;
pub mod lowest;
pub mod macd;
pub mod rsi;
pub mod sma;
pub mod volume_sma;
