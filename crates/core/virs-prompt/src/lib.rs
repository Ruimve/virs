//! Prompt 基础设施：模板加载、渲染、校验、写入。
//!
//! 从 `virs-tactical-bot` 提取，作为 core 层共享基础设施。
//! 同时定义 `PromptProvider` / `StrategyHotSwapSource` trait，
//! 让 `virs-trading-bot` 无需依赖 `virs-tactical-bot` 的具体类型。

mod loader;
mod placeholder;
mod render;
mod template;
mod validator;
mod writer;

pub use loader::{PromptLoader, ENV_STRATEGIES_DIR};
pub use placeholder::{
    all, names, to_prompt_text, Category, ContextField, Format, PlaceholderDef, PlaceholderSource,
};
pub use render::{format_bars_outside, render, RenderContext};
pub use template::{MetaFile, PromptSource, PromptTemplate};
pub use validator::{extract_placeholders, validate};
pub use writer::{delete_template, save_template};

// ── Trait 抽象 ──

use async_trait::async_trait;
use tokio::sync::watch;
use virs_type::StrategyType;

/// Prompt 提供者 trait。
///
/// `virs-trading-bot` 通过此 trait 获取策略 prompt，无需依赖 `virs-tactical-bot` 的 `PromptLoader` 具体类型。
#[async_trait]
pub trait PromptProvider: Send + Sync {
    async fn get_prompt(&self, strategy_type: StrategyType, name: &str) -> Option<PromptTemplate>;
}

#[async_trait]
impl PromptProvider for PromptLoader {
    async fn get_prompt(&self, strategy_type: StrategyType, name: &str) -> Option<PromptTemplate> {
        self.get(strategy_type, name).await
    }
}

/// 策略热切换事件。
///
/// `StrategyEngine` 完成优化后通过 watch channel 发送此事件，
/// 通知 `virs-trading-bot` 策略已更新。
#[derive(Debug, Clone)]
pub struct StrategySwapEvent {
    pub strategy_name: String,
    pub old_version: i32,
    pub new_version: i32,
}

/// 策略热切换源 trait。
///
/// `virs-trading-bot` 通过此 trait 订阅策略热切换通知，无需依赖 `virs-tactical-bot` 的 `StrategyEngine` 具体类型。
pub trait StrategyHotSwapSource: Send + Sync {
    fn subscribe(&self) -> watch::Receiver<Option<StrategySwapEvent>>;
}

#[cfg(test)]
mod loader_tests;
#[cfg(test)]
mod render_tests;
#[cfg(test)]
mod validator_tests;
#[cfg(test)]
mod writer_tests;
