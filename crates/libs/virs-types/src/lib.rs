//! VIRS unified types and trait definitions.
//!
//! This crate contains all shared types, enums, structs, and traits
//! used across the VIRS platform. All other crates depend on this
//! crate for type definitions, eliminating duplicate type definitions.

pub mod enums;
pub mod market;
pub mod position;
pub mod bot;
pub mod exchange_pe;
pub mod config;

// Re-export commonly used types
pub use enums::*;
pub use market::*;
pub use position::*;
pub use bot::*;
