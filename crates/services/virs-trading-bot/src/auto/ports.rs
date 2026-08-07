use virs_error::{BotError, BotResult};

#[derive(Debug, Clone)]
pub(crate) struct AutoMarketSnapshot {
    pub(crate) base: virs_type::MarketSnapshot,
    pub(crate) indicators: virs_indicator::IndicatorSet,
}

impl AutoMarketSnapshot {
    pub(crate) fn from_base(snapshot: virs_type::MarketSnapshot) -> BotResult<Self> {
        let indicators: virs_indicator::IndicatorSet =
            serde_json::from_value(snapshot.indicators_json.clone()).map_err(|e| {
                BotError::Validation(format!(
                    "Failed to deserialize indicators_json: {}. \
                     LLM decisions cannot be made with corrupted indicator data.",
                    e
                ))
            })?;
        Ok(Self {
            base: snapshot,
            indicators,
        })
    }
}
