//! VIRS unified types and trait definitions.
//!
//! This crate contains all shared types, enums, structs, and traits
//! used across the VIRS platform. All other crates depend on this
//! crate for type definitions, eliminating duplicate type definitions.

pub mod auto_port;
pub mod bot;
pub mod enums;
pub mod exchange_pe;
pub mod grid_port;
pub mod market;
pub mod position;

// Re-export commonly used types
pub use bot::*;
pub use enums::*;
pub use market::*;
pub use position::*;

// ============================================================
// Test modules (_tests suffix pattern)
// ============================================================
#[cfg(test)]
mod enums_tests;
#[cfg(test)]
mod market_tests;
#[cfg(test)]
mod position_tests;
#[cfg(test)]
mod bot_tests;
#[cfg(test)]
mod auto_port_tests;
#[cfg(test)]
mod serde_tests;
