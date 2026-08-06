//! 策略引擎模块：定时评估 + LLM 优化 + 热切换。
//!
//! 模块结构：
//! - [`types`]：策略指标、配置、交易记录等数据结构
//! - [`evaluator`]：从交易历史计算绩效指标
//! - [`optimizer`]：基于绩效指标通过 LLM 优化策略 prompt
//! - [`strategy_engine`]：定时循环主体，协调评估 → 优化 → 持久化 → 热切换

pub mod evaluator;
pub mod optimizer;
pub mod strategy_engine;
pub mod types;

pub use evaluator::{StrategyEvaluator, TradeHistoryProvider};
pub use optimizer::{OptimizationResult, StrategyOptimizer};
pub use strategy_engine::StrategyEngine;
pub use types::{StrategyEngineConfig, StrategyMetrics, StrategyUpdate, TradeRecord};
