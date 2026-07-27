use uuid::Uuid;

pub use virs_types::auto_port::AutoBotConfig;

pub use virs_models::AutoBot;

#[derive(Debug)]
pub enum AutoCommand {
    StartBot { bot_id: Uuid },
    StopBot { bot_id: Uuid },
    DeleteBot { bot_id: Uuid, close_position: bool },
}
