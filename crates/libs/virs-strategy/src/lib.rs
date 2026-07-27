//! 策略共性基础设施层：被 auto / grid bot 共享。
//!
//! 目录结构：
//! - [`indicator`]：统一指标库（IndicatorSpec 声明 + IndicatorSet 批量计算）
//! - [`prompt`]：策略 prompt 模板管理（文件加载 + 校验 + 占位符白名单）
//! - [`output`]：统一策略输出类型（StrategyAction + StrategyOutput + ToStrategyOutput）
//! - [`market`]：市场指标快照（MarketIndicators）+ 指标计算入口

pub mod indicator;
pub mod llm_client;
pub mod market;
pub mod output;
pub mod prompt;

#[cfg(test)]
mod output_tests;
