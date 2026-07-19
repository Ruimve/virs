//! 策略 prompt 模板管理。
//!
//! 设计目标：
//! - AI 生成 prompt 写入 `strategies/auto/*.json` 或 `strategies/grid/*.json`
//! - [`PromptLoader`] 启动时扫描 `STRATEGIES_DIR` 环境变量指向的目录，
//!   按 `strategy_type/name.json` 建立缓存
//! - worker 的 `build_llm_prompt` 优先查 loader，缺失或未配置时回退到
//!   crate 内硬编码的 `DEFAULT_*` 常量（保证向后兼容）
//!
//! 模块组成：
//! - [`template`]：数据结构（PromptTemplate / StrategyType / PromptSource）
//! - [`validator`]：占位符白名单 + 校验逻辑
//! - [`loader`]：文件扫描 + 缓存 + 查询
//! - [`ai_generator`]：AI 生成器（LLM → PromptTemplate）
//! - [`writer`]：文件写入器（PromptTemplate → JSON 文件）

pub mod ai_generator;
pub mod loader;
pub mod template;
pub mod validator;
pub mod writer;

pub use ai_generator::{generate_prompt, GenerateRequest, GenerateResult};
pub use loader::PromptLoader;
pub use template::{PromptSource, PromptTemplate, StrategyType};
pub use validator::ValidationError;
pub use writer::{delete_template, save_template};
