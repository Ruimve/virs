mod ai;
mod engine;
mod ports;
mod strategy;
mod worker;

pub use ai::{BotAction, BotDecision};
pub use engine::create_bot_engine;
pub use strategy::{
    compute_cooldown_secs, compute_position_pct, compute_stop_loss,
    compute_take_profit, format_position_info, format_stop_take_profit,
};
pub use virs_type::BotCommand;

#[cfg(test)]
mod ai_tests;
#[cfg(test)]
mod strategy_tests;
