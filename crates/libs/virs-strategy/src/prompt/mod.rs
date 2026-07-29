//! 策略 prompt 模板管理。
//!
//! 设计目标：
//! - AI 生成 prompt 写入 `strategies/auto/{name}/` 文件夹
//! - [`PromptLoader`] 启动时扫描 `STRATEGIES_DIR` 环境变量指向的目录，
//!   按 `strategy_type/name/` 文件夹建立缓存
//! - worker 的 `build_llm_prompt` 优先查 loader，缺失或未配置时回退到
//!   crate 内硬编码的 `DEFAULT_*` 常量（保证向后兼容）
//!
//! 磁盘格式：每个策略一个文件夹，内含：
//! - `meta.json` — 元数据（name, strategy_type, required_placeholders, source, ...）
//! - `system_prompt.md` — system prompt 原文（可直接编辑查看）
//! - `user_prompt_template.md` — user prompt 模板原文
//!
//! 模块组成：
//! - [`template`]：数据结构（PromptTemplate / MetaFile / StrategyType / PromptSource）
//! - [`validator`]：占位符白名单 + 校验逻辑
//! - [`loader`]：文件夹扫描 + 缓存 + 查询
//! - [`ai_generator`]：AI 生成器（LLM → PromptTemplate）
//! - [`writer`]：文件夹写入器（PromptTemplate → 3 文件）
//! - [`render`]：统一 prompt 渲染器（RenderContext + render）

pub mod ai_generator;
pub mod loader;
pub mod render;
pub mod template;
pub mod validator;
pub mod writer;

pub use ai_generator::{generate_prompt, GenerateRequest, GenerateResult};
pub use loader::PromptLoader;
pub use render::{render, RenderContext};
pub use template::{MetaFile, PromptSource, PromptTemplate, StrategyType};
pub use writer::{delete_template, save_template};

#[cfg(test)]
mod ai_generator_tests;
#[cfg(test)]
mod loader_tests;
#[cfg(test)]
mod render_tests;
#[cfg(test)]
mod validator_tests;
#[cfg(test)]
mod writer_tests;
