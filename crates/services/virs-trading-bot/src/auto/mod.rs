mod ai;
mod engine;
mod ports;
mod strategy;
mod worker;

pub use ai::{AutoAction, AutoDecision};
pub use engine::create_auto_engine;
pub use strategy::{
    compute_cooldown_secs, compute_position_pct, compute_stop_loss,
    compute_take_profit, compute_trailing_stop, format_position_info,
    format_stop_take_profit,
};
pub use virs_type::AutoCommand;

#[cfg(test)]
mod ai_tests;
#[cfg(test)]
mod strategy_tests;
