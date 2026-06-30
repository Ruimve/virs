//! VIRS database models.
//!
//! These types map directly to database rows (sqlx FromRow).
//! Enum types (Side, PositionSide, MarketType, etc.) are re-exported from virs-types
//! to avoid duplicate definitions.
//! Shared domain types (Kline, ApiResponse, PaginationParams, ExchangePosition, etc.)
//! are also re-exported from virs-types::market.

pub mod auto;
pub mod grid;
pub mod trading;
pub mod user;

// Re-export unified types from virs-types
pub use auto::{AutoBot, AutoTrade};
pub use grid::{GridBot, GridTrade};
pub use trading::Order;
pub use user::{CreateUserRequest, LoginRequest, LoginResponse, User, UserResponse};
pub use virs_types::enums::*;
pub use virs_types::market::*;

// ============================================================
// Test modules (_tests suffix pattern)
// ============================================================
#[cfg(test)]
mod trading_tests;
#[cfg(test)]
mod user_tests;
#[cfg(test)]
mod grid_tests;
#[cfg(test)]
mod auto_tests;
#[cfg(test)]
mod serde_tests;
