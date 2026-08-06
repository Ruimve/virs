//! 技术指标计算库。
//!
//! 职责：
//! - [`IndicatorSpec`]：声明式指标规格（策略通过它声明所需指标）
//! - [`IndicatorSet`]：批量去重计算 + 查询
//! - [`compute_indicators`]：统一计算入口
//!
//! 本 crate 是纯计算层，不依赖 prompt / LLM / 异步运行时。

mod compute;
mod indicators;
mod set;
mod spec;

// 向后兼容：重导出原子函数供外部直接调用
// （如 virs-api 的 strategy_selection.rs）
pub use indicators::atomic::*;

pub use compute::compute_indicators;
pub use set::{IndicatorSet, IndicatorValue, KlineSet};
pub use spec::IndicatorSpec;
