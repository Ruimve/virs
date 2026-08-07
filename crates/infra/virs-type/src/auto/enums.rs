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


#[derive(Debug)]
pub enum AutoCommand {
    StartBot { bot_id: Uuid },
    StopBot { bot_id: Uuid },
    DeleteBot {
        bot_id: Uuid,
        close_position: bool,


        response_tx: oneshot::Sender<Result<(), String>>,
    },
}
