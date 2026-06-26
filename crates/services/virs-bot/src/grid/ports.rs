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

/// 市场快照（grid 专用扩展，包含指标）
#[derive(Debug, Clone, Default)]
pub struct GridMarketSnapshot {
    pub base: virs_types::bot::MarketSnapshot,
    pub indicators: crate::common::indicators::MarketIndicators,
}

impl GridMarketSnapshot {
    /// Convert a base MarketSnapshot into GridMarketSnapshot by deserializing indicators_json.
    pub fn from_base(snapshot: virs_types::bot::MarketSnapshot) -> Self {
        let indicators: crate::common::indicators::MarketIndicators =
            serde_json::from_value(snapshot.indicators_json.clone()).unwrap_or_default();
        Self {
            base: snapshot,
            indicators,
        }
    }
}
