//! virs-bot — Bot service (grid + auto trading).
//!
//! Contains:
//! - `grid`: Semi-automatic grid trading bot
//! - `auto`: Fully automatic trading bot
//! - `common`: Shared ports, types, AI client, indicators

pub mod auto;
pub mod common;
pub mod grid;

// Re-export key types
pub use auto::AutoEngine;
pub use grid::GridEngine;
