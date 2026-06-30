//! VIRS utility functions: authentication and cryptography.

pub mod auth;
pub mod crypto;

// ============================================================
// Test modules (_tests suffix pattern)
// ============================================================
#[cfg(test)]
mod auth_tests;
#[cfg(test)]
mod crypto_tests;
