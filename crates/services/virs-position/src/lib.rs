//! virs-position — Position Engine service.
//!
//! Manages positions, orders, PnL tracking, and persistence.

pub mod engine;
pub mod persistence;
pub mod tracker;

// Re-export key types
pub use engine::PositionEngine;
pub use persistence::{Persistence, PositionPersistence};
pub use tracker::{calc_drawdown_pct, calc_unrealized_pnl, PnlTracker};

#[cfg(test)]
mod tracker_tests;
