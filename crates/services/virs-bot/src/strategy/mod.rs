//! 策略共性基础设施层 — 从 `virs-strategy` crate 重新导出。
//!
//! 策略 prompt 模板管理、指标库、统一输出类型等已抽取到独立的
//! `virs-strategy` crate（`crates/libs/virs-strategy`）。
//! 本模块仅做重新导出，保持 `virs_bot::strategy::*` 路径向后兼容。
//!
//! Auto 专属的交易数学函数（止损止盈、移动止损、仓位百分比、冷却时间）
//! 保留在 [`crate::auto::strategy`] 中，不随 `virs-strategy` 抽取。

pub use virs_strategy::indicator;
pub use virs_strategy::output;
pub use virs_strategy::prompt;

// 重新导出常用类型，保持现有 `use crate::strategy::X` 引用不变
pub use virs_strategy::output::{StrategyAction, StrategyOutput, ToStrategyOutput};
pub use virs_strategy::prompt::{
    delete_template, generate_prompt, render, GenerateRequest, GenerateResult, MetaFile,
    PromptLoader, PromptSource, PromptTemplate, StrategyType, ValidationError,
};

/// 市场指标快照 — 从 `virs-strategy::market` 重新导出。
pub use virs_strategy::market::{compute_market_indicators, MarketIndicators};

/// LLM 客户端 — 从 `virs-strategy::llm_client` 重新导出。
pub use virs_strategy::llm_client::{call_llm_api, create_llm_http_client, LlmCallResult};
