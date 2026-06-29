//! VIRS configuration management.

mod app_config;

pub use app_config::*;

// ============================================================
// Test modules (_tests suffix pattern)
// ============================================================
#[cfg(test)]
mod app_config_tests;
#[cfg(test)]
mod serde_tests;
