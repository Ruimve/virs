

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use virs_type::StrategyType;


/* 提示词来源：Human（人工编写）或 AiGenerated（AI 生成，记录模型和生成提示词） */
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
#[derive(Default)]
pub enum PromptSource {

    #[default]
    Human,

    AiGenerated {
        model: String,
        generation_prompt: String,
    },
}


/* 策略提示词模板：包含系统提示词、用户提示词模板和必需占位符列表，是 AI 交易决策的核心输入 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {

    pub name: String,

    pub strategy_type: StrategyType,

    pub system_prompt: String,

    pub user_prompt_template: String,


    pub required_placeholders: Vec<String>,

    #[serde(default)]
    pub source: PromptSource,

    #[serde(default = "default_version")]
    pub version: i32,

    #[serde(default)]
    pub description: String,

    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
}


/* 元数据文件结构：meta.json 的序列化模型，不含提示词正文，仅记录模板元信息 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaFile {

    pub name: String,

    pub strategy_type: StrategyType,

    pub required_placeholders: Vec<String>,

    #[serde(default)]
    pub source: PromptSource,

    #[serde(default = "default_version")]
    pub version: i32,

    #[serde(default)]
    pub description: String,

    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
}

impl MetaFile {

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
