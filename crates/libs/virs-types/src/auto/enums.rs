use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(feature = "sqlx", sqlx(type_name = "text", rename_all = "lowercase"))]
#[serde(rename_all = "lowercase")]
pub enum StrategyStatus {
    Draft,
    Running,
    Paused,
    Stopped,
    Error,
}


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
