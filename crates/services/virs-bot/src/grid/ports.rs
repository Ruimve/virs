//! Grid bot port definitions.
//!
//! All trait and data type definitions are in virs-types::grid_port and virs-types::bot.
//! This module re-exports them for convenience within the grid module.

// Re-export all grid port types from virs-types
pub use virs_types::grid_port::*;

// Re-export unified bot-layer traits and types
pub use virs_types::bot::{
    AccountBalance, BotPositionSide, MarketDataProvider, MarketSnapshot, OrderCommand, OrderEvent,
    OrderExecutor, OrderInfo, OrderSide, PriceProvider,
};
