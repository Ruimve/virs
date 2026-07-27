//! 统一指标库。
//!
//! 设计目标：
//! - 策略通过 [`IndicatorSpec`] 声明所需指标，主程序统一计算注入
//! - [`IndicatorSet::compute`] 去重批量计算，避免重复求值
//! - 缺失指标通过 `get` 返回 `None`，禁止隐式默认值（符合 virs-error 约束）
//!
//! 迁移说明：原子计算函数从 `common/indicators.rs` 迁移至 [`library`]，
//! `common::indicators` 现作为薄包装层转发，保持向后兼容。

pub mod library;
pub mod set;
pub mod spec;

pub use library::*;
pub use set::{all_market_indicators_specs, IndicatorSet, IndicatorValue, KlineSet};
pub use spec::{IndicatorSpec, Timeframe};
