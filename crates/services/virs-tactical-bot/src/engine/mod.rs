mod evaluator;
mod optimizer;
mod strategy_engine;
mod types;

pub use evaluator::{StrategyEvaluator, TradeHistoryProvider};
pub use optimizer::{OptimizationResult, StrategyOptimizer};
pub use strategy_engine::StrategyEngine;
pub use types::{StrategyEngineConfig, StrategyMetrics, TradeRecord};
