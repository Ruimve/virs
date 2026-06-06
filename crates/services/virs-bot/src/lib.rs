//! virs-bot — Bot service (grid + auto trading).
//!
//! Contains:
//! - `grid`: Semi-automatic grid trading bot
//! - `auto`: Fully automatic trading bot
//! - `common`: Shared ports, types, AI client, indicators

pub mod common;
pub mod grid;
pub mod auto;

// Re-export key types
pub use grid::GridEngine;
pub use auto::AutoEngine;
