mod ai;
mod engine;
mod ports;
mod strategy;
mod types;
mod worker;

pub use ai::{AutoAction, AutoAiService, AutoDecision};
pub use engine::AutoEngine;
pub use strategy::{
    compute_cooldown_secs, compute_position_pct, compute_stop_loss,
    compute_take_profit, compute_trailing_stop, format_position_info,
    format_stop_take_profit,
};
pub use types::AutoCommand;

#[cfg(test)]
mod ai_tests;
#[cfg(test)]
mod strategy_tests;
