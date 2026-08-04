//! 指标计算实现。
//!
//! 分三层：
//! - [`atomic`]：TA-Lib 直接封装（Layer 0）
//! - [`derived`]：组合原子函数的派生指标（Layer 1+2）
//! - [`primitive`]：直接读 K 线字段的原始指标

pub mod atomic;
pub mod derived;
pub mod primitive;

#[cfg(test)]
pub mod test_utils;

use virs_types::Kline;

/// 提取收盘价序列。
pub fn closes(klines: &[Kline]) -> Vec<f64> {
    klines.iter().map(|k| k.close).collect()
}

/// 提取最高价序列。
pub fn highs(klines: &[Kline]) -> Vec<f64> {
    klines.iter().map(|k| k.high).collect()
}

/// 提取最低价序列。
pub fn lows(klines: &[Kline]) -> Vec<f64> {
    klines.iter().map(|k| k.low).collect()
}
