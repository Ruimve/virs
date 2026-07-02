//! virs-market — market data collection and aggregation service.
//!
//! Provides KlineEngine for real-time kline data via WebSocket,
//! automatic gap detection and backfill, and multi-timeframe aggregation.
//! Also provides OrderBookEngine for real-time order book streaming.

pub mod aggregator;
pub mod cache;
pub mod engine;
pub mod gap;
pub mod orderbook_engine;
pub mod source;
pub mod types;

// Re-export key types for convenience
pub use engine::KlineEngine;
pub use gap::ContinuityReport;
pub use orderbook_engine::OrderBookEngine;
pub use source::ExchangeKlineSource;
pub use types::{
    align_open_time, subscription_key, AllTimeframesData, Candle, KlineEngineConfig, KlineEvent,
    KlineEventType, KlinePersistence, KlineSource, KlineWsClient, MarketType,
    OrderBookEngineConfig, OrderBookEvent, OrderBookLevel, OrderBookWsClient, Timeframe,
    WsCandleUpdate, WsEvent, WsOrderBookEvent, WsOrderBookUpdate,
};

#[cfg(test)]
mod aggregator_tests;
#[cfg(test)]
mod cache_tests;
#[cfg(test)]
mod types_tests;
