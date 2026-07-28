use uuid::Uuid;

#[derive(Debug)]
pub enum AutoCommand {
    StartBot { bot_id: Uuid },
    StopBot { bot_id: Uuid },
    DeleteBot { bot_id: Uuid, close_position: bool },
}
