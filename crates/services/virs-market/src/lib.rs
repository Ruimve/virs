mod aggregator;
mod cache;
mod engine;
mod gap;
mod orderbook_engine;
mod source;
mod types;

pub use aggregator::candle_from_1m;
pub use engine::create_kline_engine;
pub use orderbook_engine::create_orderbook_engine;
pub use source::{create_exchange_kline_source, timeframe_str_to_ms};
pub use types::{
    align_open_time, subscription_key, KlineEngineConfig,
    KlineSource, OrderBookEngineConfig,
};

#[cfg(test)]
mod aggregator_tests;
#[cfg(test)]
mod cache_tests;
#[cfg(test)]
mod types_tests;
