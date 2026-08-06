mod aggregator;
mod cache;
mod engine;
mod gap;
mod orderbook_engine;
mod source;
mod types;

pub use aggregator::{candle_from_1m, Aggregator};
pub use cache::SymbolCache;
pub use engine::KlineEngine;
pub use gap::ContinuityReport;
pub use orderbook_engine::OrderBookEngine;
pub use source::{timeframe_str_to_ms, ExchangeKlineSource};
pub use types::{
    align_open_time, subscription_key, KlineEngineConfig,
    KlinePersistence, KlineSource, OrderBookEngineConfig, OrderBookEvent,
};

#[cfg(test)]
mod aggregator_tests;
#[cfg(test)]
mod cache_tests;
#[cfg(test)]
mod types_tests;
