use tokio::sync::oneshot;
use uuid::Uuid;

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
