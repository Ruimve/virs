//! Exchange-based KlineSource implementation.
//!
//! Uses virs-exchange's Exchange trait to fetch klines from exchange REST API.

use async_trait::async_trait;
use virs_types::enums::MarketType;
use virs_exchange::Exchanges;

use crate::types::{Candle, KlineSource};

pub struct ExchangeKlineSource {
    registry: std::sync::Arc<Exchanges>,
}

impl ExchangeKlineSource {
    pub fn new(registry: std::sync::Arc<Exchanges>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl KlineSource for ExchangeKlineSource {
    async fn fetch_klines(
        &self,
        exchange: &str,
        symbol: &str,
        timeframe: &str,
        limit: u32,
        since: Option<i64>,
        market_type: Option<MarketType>,
    ) -> anyhow::Result<Vec<Candle>> {
        // Try to find exchange by name with market type suffix
        let key = if let Some(mt) = market_type {
            let key = format!("{}:{}", exchange, mt);
            if self.registry.get(&key).is_some() {
                key
            } else {
                // Fallback: find by prefix
                self.registry.registered_names()
                    .into_iter()
                    .find(|n| n.starts_with(&format!("{}:", exchange)))
                    .ok_or_else(|| anyhow::anyhow!("Exchange '{}' not found in registry", exchange))?
            }
        } else {
            self.registry.registered_names()
                .into_iter()
                .find(|n| n.starts_with(&format!("{}:", exchange)))
                .ok_or_else(|| anyhow::anyhow!("Exchange '{}' not found in registry", exchange))?
        };

        let ex = self.registry.get(&key)
            .ok_or_else(|| anyhow::anyhow!("Exchange '{}' not available", exchange))?;

        let klines = ex.get_klines(symbol, timeframe, limit, since).await?;

        // Convert virs_models Kline to virs_ccxt Candle
        Ok(klines.into_iter().map(|k| Candle {
            open_time: k.open_time,
            close_time: k.open_time + match timeframe {
                "1m" => 60_000,
                "5m" => 300_000,
                "15m" => 900_000,
                "1h" => 3_600_000,
                "4h" => 14_400_000,
                "1d" => 86_400_000,
                _ => 60_000,
            } - 1,
            open: k.open,
            high: k.high,
            low: k.low,
            close: k.close,
            volume: k.volume,
            quote_volume: k.quote_volume,
            trades: k.trades,
            closed: true,
        }).collect())
    }
}
