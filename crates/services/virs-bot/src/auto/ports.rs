//! Auto trading bot port definitions.
//!
//! All trait and data type definitions are in virs-types::auto_port and virs-types::bot.
//! This module re-exports them for convenience within the auto module.

// Re-export all auto port types from virs-types
pub use virs_types::auto_port::*;

// Re-export unified bot-layer traits and types
pub use virs_types::bot::{
    PriceProvider, MarketDataProvider, MarketSnapshot, AccountBalance,
    OrderExecutor, OrderEvent, OrderSide, BotPositionSide, OrderInfo, OrderCommand,
};

/// 市场快照（auto 专用扩展，包含指标）
#[derive(Debug, Clone, Default)]
pub struct AutoMarketSnapshot {
    pub base: virs_types::bot::MarketSnapshot,
    pub indicators: crate::common::indicators::MarketIndicators,
}

impl AutoMarketSnapshot {
    /// Convert a base MarketSnapshot into AutoMarketSnapshot by deserializing indicators_json.
    pub fn from_base(snapshot: virs_types::bot::MarketSnapshot) -> Self {
        let indicators: crate::common::indicators::MarketIndicators =
            serde_json::from_value(snapshot.indicators_json.clone()).unwrap_or_default();
        Self { base: snapshot, indicators }
    }
}
