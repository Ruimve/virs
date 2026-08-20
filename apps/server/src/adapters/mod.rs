mod bot_store;
mod credential_store;
mod llm_resolver;
mod market_data;
mod order_executor;
mod trade_history;

pub use bot_store::{PgBotStore, bot_to_config};
pub use credential_store::PgCredentialStore;
pub use llm_resolver::{DefaultLlmResolver, resolve_llm_provider};
pub use market_data::{AutoExchangeMarketDataProvider, candle_to_kline};
pub use order_executor::{PeOrderExecutor, convert_pe_event};
pub use trade_history::PgTradeHistoryProvider;

#[cfg(test)]
mod bot_store_tests;
#[cfg(test)]
mod llm_resolver_tests;
#[cfg(test)]
mod market_data_tests;
#[cfg(test)]
mod order_executor_tests;
