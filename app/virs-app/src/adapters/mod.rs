//! Adapters — connect external services to virs-bot ports.
//!
//! All adapters are implemented here in virs-app (the composition root)
//! because only virs-app has access to all crate dependencies.

mod grid_store;
mod auto_store;
mod price_provider;
mod market_data;
mod order_executor;
mod credential_store;
mod llm_resolver;

pub use grid_store::PgGridStore;
pub use auto_store::PgAutoStore;
pub use price_provider::{ExchangePriceProvider, AutoExchangePriceProvider};
pub use market_data::{ExchangeMarketDataProvider, AutoExchangeMarketDataProvider};
pub use order_executor::PeOrderExecutor;
pub use credential_store::PgCredentialStore;
pub use llm_resolver::DefaultLlmResolver;
