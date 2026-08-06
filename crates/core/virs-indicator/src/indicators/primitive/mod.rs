//! 原始指标：直接读 K 线字段，不调用 TA-Lib。

pub mod candle_body;
pub mod change_pct;
pub mod current_price;
pub mod last_volume;

#[cfg(test)]
mod candle_body_tests;
#[cfg(test)]
mod change_pct_tests;
#[cfg(test)]
mod current_price_tests;
#[cfg(test)]
mod last_volume_tests;
