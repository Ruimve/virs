//! virs-market — market data collection and aggregation service.
//!
//! Provides KlineEngine for real-time kline data via WebSocket,
//! automatic gap detection and backfill, and multi-timeframe aggregation.

pub mod types;
pub mod cache;
pub mod aggregator;
pub mod gap;
pub mod source;
pub mod engine;

// Re-export key types for convenience
pub use engine::KlineEngine;
pub use source::ExchangeKlineSource;
pub use types::{
    Timeframe, KlineEvent, KlineEventType, Candle, WsEvent, WsCandleUpdate,
    AllTimeframesData, KlineEngineConfig, KlineSource, KlinePersistence,
    KlineWsClient, MarketType, BacktestRangeLimit, BacktestRangeInfo,
    subscription_key, align_open_time,
};
pub use gap::ContinuityReport;
