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
pub use risk::{DrawdownAction, RiskAlertInfo, RiskChecker};
pub use tracker::{PnlSnapshot, PnlTracker};
