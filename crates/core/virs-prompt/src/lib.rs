

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
pub use render::{render, RenderContext};
pub use template::{MetaFile, PromptTemplate};
pub use validator::{extract_placeholders, validate};
pub use writer::{create_strategy, delete_strategy, save_new_version};


use async_trait::async_trait;
use tokio::sync::watch;
use virs_type::StrategyType;


/* 策略提示词提供者 trait：抽象提示词获取逻辑，支持 PromptLoader 实现和自定义实现 */
#[async_trait]
pub trait PromptProvider: Send + Sync {
    async fn get_prompt(&self, strategy_type: StrategyType, name: &str) -> Option<PromptTemplate>;

    async fn get_output_format(&self) -> Option<String>;
}

#[async_trait]
impl PromptProvider for PromptLoader {
    async fn get_prompt(&self, strategy_type: StrategyType, name: &str) -> Option<PromptTemplate> {
        self.get(strategy_type, name).await
    }

    async fn get_output_format(&self) -> Option<String> {
        self.output_format().await
    }
}


#[derive(Debug, Clone)]
pub struct StrategySwapEvent {
    pub strategy_name: String,
    pub old_version: i32,
    pub new_version: i32,
}


/* 策略热替换事件源 trait：通过 watch channel 通知策略版本变更，实现运行时热更新 */
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
