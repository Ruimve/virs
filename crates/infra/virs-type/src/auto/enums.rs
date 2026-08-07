use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use uuid::Uuid;


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

/// 自动交易引擎控制命令。
#[derive(Debug)]
pub enum AutoCommand {
    StartBot { bot_id: Uuid },
    StopBot { bot_id: Uuid },
    DeleteBot {
        bot_id: Uuid,
        close_position: bool,
        /// Response channel for the handler to await engine completion.
        /// `Ok(())` = bot deleted successfully; `Err(msg)` = engine failed mid-deletion.
        response_tx: oneshot::Sender<Result<(), String>>,
    },
}
