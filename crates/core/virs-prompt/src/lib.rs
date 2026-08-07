

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


use async_trait::async_trait;
use tokio::sync::watch;
use virs_type::StrategyType;


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


#[derive(Debug, Clone)]
pub struct StrategySwapEvent {
    pub strategy_name: String,
    pub old_version: i32,
    pub new_version: i32,
}


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
