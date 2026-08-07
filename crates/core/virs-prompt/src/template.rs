

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use virs_type::StrategyType;


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
