//! virs-position — Position Engine service.
//!
//! Manages positions, orders, risk checks, PnL tracking, and persistence.

pub mod engine;
pub mod persistence;
pub mod risk;
pub mod tracker;

// Re-export key types
pub use engine::PositionEngine;
pub use persistence::{Persistence, PositionPersistence};
pub use risk::{
    calc_symbol_exposure, calc_total_exposure, check_drawdown, check_funding_rate,
    check_liquidation, DrawdownAction, RiskAlertInfo, RiskChecker,
};
pub use tracker::{calc_drawdown_pct, calc_unrealized_pnl, PnlTracker};

#[cfg(test)]
mod risk_tests;
#[cfg(test)]
mod tracker_tests;
