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
pub use virs_types::enums::*;
pub use virs_types::market::*;
