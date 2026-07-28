pub mod aggregator;
pub mod cache;
pub mod engine;
pub mod gap;
pub mod orderbook_engine;
pub mod source;
pub mod types;

pub use engine::KlineEngine;
pub use gap::ContinuityReport;
pub use orderbook_engine::OrderBookEngine;
pub use source::ExchangeKlineSource;
pub use types::{
    align_open_time, subscription_key, KlineEngineConfig, KlineEvent, KlineEventType,
    KlinePersistence, KlineSource, OrderBookEngineConfig, OrderBookEvent, Timeframe,
};

#[cfg(test)]
mod aggregator_tests;
#[cfg(test)]
mod cache_tests;
#[cfg(test)]
mod types_tests;
