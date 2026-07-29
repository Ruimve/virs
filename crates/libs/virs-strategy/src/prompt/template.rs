//! Prompt 模板数据结构。
//!
//! 一个 [`PromptTemplate`] 对应 `strategies/auto/{name}/` 一个文件夹，
//! 内含 `meta.json` + `system_prompt.md` + `user_prompt_template.md` 三个文件。
//! API 传输时仍使用单个 JSON（[`PromptTemplate`] 本身），仅磁盘存储拆分为三文件。
//! 字段设计与 AI 生成产物对齐：LLM 生成 JSON → 校验 → 写文件夹 → loader 加载。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 策略类型。对应 `strategies/auto/{name}/` 子目录。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StrategyType {
    Auto,
}

impl StrategyType {
    pub fn as_dir(&self) -> &'static str {
        match self {
            StrategyType::Auto => "auto",
        }
    }
}

/// Prompt 来源标记。AI 生成时记录模型与元 prompt，便于追溯。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum PromptSource {
    /// 人工编写或 crate 内置默认值
    Human,
    /// AI 生成。记录生成模型与生成时的元 prompt（描述意图）
    AiGenerated {
        model: String,
        generation_prompt: String,
    },
}

impl Default for PromptSource {
    fn default() -> Self {
        PromptSource::Human
    }
}

/// Prompt 模板（内存表示，同时用于 API JSON 传输）。
///
/// 磁盘存储为文件夹 `strategies/{type}/{name}/`，内含三个文件：
/// - `meta.json` — 除 system_prompt / user_prompt_template 外的全部字段
/// - `system_prompt.md` — system_prompt 原文（Markdown，可直接编辑查看）
/// - `user_prompt_template.md` — user_prompt_template 原文
///
/// API 传输时使用完整的 [`PromptTemplate`] JSON（含所有字段）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    /// 模板名。对应文件夹名 `{name}/`
    pub name: String,
    /// 策略类型，决定文件所在子目录
    pub strategy_type: StrategyType,
    /// LLM system prompt。定义角色、规则、输出 JSON schema
    pub system_prompt: String,
    /// 用户 prompt 模板，含 `{placeholder}` 占位符
    pub user_prompt_template: String,
    /// 声明使用的占位符列表。用于：
    /// - 校验模板内 `{xxx}` 全部在白名单内
    /// - 反查所需指标（`placeholder_to_indicator`）供主程序统一计算
    pub required_placeholders: Vec<String>,
    /// 来源标记
    #[serde(default)]
    pub source: PromptSource,
    /// 版本号，人工/AI 编辑时递增
    #[serde(default = "default_version")]
    pub version: i32,
    /// 人类可读的描述
    #[serde(default)]
    pub description: String,
    /// 创建时间（ISO 8601）。缺失时由 loader 填入文件 mtime
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
}

/// 磁盘上的 `meta.json` 结构。
///
/// [`PromptTemplate`] 的子集——不包含 `system_prompt` 和 `user_prompt_template`，
/// 这两个字段单独存为 `.md` 文件以便编辑查看。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaFile {
    /// 模板名。对应文件夹名 `{name}/`
    pub name: String,
    /// 策略类型，决定文件所在子目录
    pub strategy_type: StrategyType,
    /// 声明使用的占位符列表
    pub required_placeholders: Vec<String>,
    /// 来源标记
    #[serde(default)]
    pub source: PromptSource,
    /// 版本号，人工/AI 编辑时递增
    #[serde(default = "default_version")]
    pub version: i32,
    /// 人类可读的描述
    #[serde(default)]
    pub description: String,
    /// 创建时间（ISO 8601）
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
}

impl MetaFile {
    /// 从 [`PromptTemplate`] 提取元数据（不含 prompt 文本）。
    pub fn from_template(tpl: &PromptTemplate) -> Self {
        Self {
            name: tpl.name.clone(),
            strategy_type: tpl.strategy_type,
            required_placeholders: tpl.required_placeholders.clone(),
            source: tpl.source.clone(),
            version: tpl.version,
            description: tpl.description.clone(),
            created_at: tpl.created_at,
        }
    }
}

fn default_version() -> i32 {
    1
}
