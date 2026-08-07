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


/* 自动机器人命令枚举：启动、停止、删除机器人（删除操作通过 oneshot 返回结果） */
#[derive(Debug)]
pub enum AutoCommand {
    StartBot { bot_id: Uuid },
    StopBot { bot_id: Uuid },
    DeleteBot {
        bot_id: Uuid,
        close_position: bool,

        /* oneshot 通道：删除操作完成后通过此通道返回结果 */
        response_tx: oneshot::Sender<Result<(), String>>,
    },
}
