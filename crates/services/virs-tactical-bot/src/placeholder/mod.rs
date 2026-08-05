//! 占位符注册中心 — 单一数据源。
//!
//! 将占位符名、数据来源（Context / Indicator）、格式化规则、分类统一声明在一处。
//! 消费方（validator / render / ai_generator）全部从此处取数据，不再各自硬编码。
//!
//! - [`registry::all()`]：返回全部占位符定义
//! - [`registry::names()`]：返回占位符名称集合（validator 白名单）
//! - [`registry::to_prompt_text()`]：生成 LLM 可读的占位符清单（ai_generator）

pub mod registry;

pub use registry::{all, names, to_prompt_text, Category, ContextField, Format, PlaceholderDef, PlaceholderSource};
