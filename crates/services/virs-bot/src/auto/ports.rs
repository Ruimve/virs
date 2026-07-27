pub use virs_types::auto_port::*;

pub use virs_types::bot::{
    AccountBalance, BotPositionSide, MarketDataProvider, MarketSnapshot, OrderCommand, OrderEvent,
    OrderExecutor, OrderInfo, OrderSide, PriceProvider,
};

#[derive(Debug, Clone, Default)]
pub struct AutoMarketSnapshot {
    pub base: virs_types::bot::MarketSnapshot,
    pub indicators: virs_strategy::market::MarketIndicators,
}

impl AutoMarketSnapshot {
    pub fn from_base(snapshot: virs_types::bot::MarketSnapshot) -> Self {
        let indicators: virs_strategy::market::MarketIndicators =
            serde_json::from_value(snapshot.indicators_json.clone()).unwrap_or_else(|e| {
                tracing::warn!(
                    error = %e,
                    "Failed to deserialize indicators_json — using all-zero defaults. \
                     LLM decisions based on zero indicators may be incorrect."
                );
                virs_strategy::market::MarketIndicators::default()
            });
        Self {
            base: snapshot,
            indicators,
        }
    }
}
