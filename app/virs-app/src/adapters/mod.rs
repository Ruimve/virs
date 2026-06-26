//! Adapters — connect external services to virs-bot ports.
//!
//! All adapters are implemented here in virs-app (the composition root)
//! because only virs-app has access to all crate dependencies.

mod auto_store;
mod credential_store;
mod grid_store;
mod llm_resolver;
mod market_data;
mod order_executor;
mod price_provider;

pub use auto_store::PgAutoStore;
pub use credential_store::PgCredentialStore;
pub use grid_store::PgGridStore;
pub use llm_resolver::DefaultLlmResolver;
pub use market_data::{AutoExchangeMarketDataProvider, ExchangeMarketDataProvider};
pub use order_executor::PeOrderExecutor;
pub use price_provider::{AutoExchangePriceProvider, ExchangePriceProvider};
