


use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use virs_type::StrategyType;


/* 策略提示词模板：包含系统提示词、用户提示词模板和必需占位符列表，是 AI 交易决策的核心输入 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {

    pub name: String,

    pub strategy_type: StrategyType,

    pub system_prompt: String,

    pub user_prompt_template: String,


    pub required_placeholders: Vec<String>,

    #[serde(default = "default_version")]
    pub version: i32,

    #[serde(default)]
    pub description: String,

    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
}


/* 元数据文件结构：meta.json 的序列化模型，不含提示词正文，仅记录文件路径和版本信息。
 * 路径字段相对于策略文件夹根目录，解析时拼接 v{version}/ 前缀定位实际文件。 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaFile {

    pub name: String,

    pub strategy_type: StrategyType,

    pub system_prompt: String,

    pub user_prompt: String,

    pub required_placeholders: String,

    pub description: String,

    #[serde(default = "default_version")]
    pub version: i32,

    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
}

impl MetaFile {

    pub fn from_template(tpl: &PromptTemplate) -> Self {
        Self {
            name: tpl.name.clone(),
            strategy_type: tpl.strategy_type,
            system_prompt: "./system_prompt.md".to_string(),
            user_prompt: "./user_prompt_template.md".to_string(),
            required_placeholders: "./required_placeholders.json".to_string(),
            description: "./description.md".to_string(),
            version: tpl.version,
            created_at: tpl.created_at,
        }
    }


    /* 去掉路径字段的 "./" 前缀，返回纯文件名部分。writer 和 loader 统一通过此方法解析路径，
     * 确保 meta.json 中的路径值是唯一的 source of truth */
    pub fn filename(path: &str) -> &str {
        path.strip_prefix("./").unwrap_or(path)
    }
}

fn default_version() -> i32 {
    1
}
