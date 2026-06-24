//! virs-position — Position Engine service.
//!
//! Manages positions, orders, risk checks, PnL tracking, and persistence.

pub mod engine;
pub mod risk;
pub mod tracker;
pub mod persistence;

// Re-export key types
pub use engine::PositionEngine;
pub use risk::{RiskChecker, RiskAlertInfo, DrawdownAction};
pub use tracker::{PnlTracker, PnlSnapshot};
pub use persistence::{PositionPersistence, Persistence};
