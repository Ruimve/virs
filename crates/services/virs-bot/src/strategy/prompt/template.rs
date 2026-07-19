//! Prompt 模板数据结构。
//!
//! 一个 [`PromptTemplate`] 对应 `strategies/{auto,grid}/{name}.json` 一个文件。
//! 字段设计与 AI 生成产物对齐：LLM 生成 JSON → 校验 → 写文件 → loader 加载。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 策略类型。对应 `strategies/` 下的两个子目录。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StrategyType {
    Auto,
    Grid,
}

impl StrategyType {
    pub fn as_dir(&self) -> &'static str {
        match self {
            StrategyType::Auto => "auto",
            StrategyType::Grid => "grid",
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

/// Prompt 模板。
///
/// JSON 文件结构示例：
/// ```json
/// {
///   "name": "trend_following",
///   "strategy_type": "auto",
///   "system_prompt": "你是趋势跟随引擎...",
///   "user_prompt_template": "当前时间:{timestamp}\n4h:{h4_ema20}...",
///   "required_placeholders": ["h4_ema20", "h1_rsi"],
///   "source": { "kind": "human" },
///   "version": 1,
///   "description": "多周期趋势跟随",
///   "created_at": "2026-07-19T10:00:00Z"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    /// 模板名（不含扩展名）。对应文件名 `{name}.json`
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

fn default_version() -> i32 {
    1
}
