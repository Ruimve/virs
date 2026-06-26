//! Price providers — Exchanges + KlineEngine backed price lookups.

use std::sync::Arc;

use async_trait::async_trait;

use virs_exchange::Exchanges;
use virs_market::KlineEngine;
use virs_market::Timeframe;
use virs_types::bot::PriceProvider;

// ── Grid PriceProvider ──

pub struct ExchangePriceProvider {
    exchange_registry: Arc<Exchanges>,
    kline_engine: Option<Arc<KlineEngine>>,
}

impl ExchangePriceProvider {
    pub fn new(exchange_registry: Arc<Exchanges>) -> Self {
        Self {
            exchange_registry,
            kline_engine: None,
        }
    }

    pub fn with_kline_engine(mut self, engine: Arc<KlineEngine>) -> Self {
        self.kline_engine = Some(engine);
        self
    }
}

#[async_trait]
impl PriceProvider for ExchangePriceProvider {
    async fn get_price(&self, exchange: &str, symbol: &str, market_type: &str) -> Option<f64> {
        // Try kline engine first (1m candle)
        if let Some(ref engine) = self.kline_engine {
            if let Some(candles) = engine
                .get_klines_async(exchange, symbol, Timeframe::M1)
                .await
            {
                if let Some(last) = candles.last() {
                    if last.close > 0.0 {
                        return Some(last.close);
                    }
                }
            }
        }

        // Fallback to exchange ticker
        let exchange_key = format!("{}:{}", exchange, market_type);
        let ex = self.exchange_registry.get(&exchange_key)?;
        match ex.get_ticker(symbol).await {
            Ok(ticker) if ticker.last > 0.0 => Some(ticker.last),
            _ => {
                // Also try spot if perpetual failed
                if market_type != "spot" {
                    let spot_key = format!("{}:spot", exchange);
                    if let Some(ex) = self.exchange_registry.get(&spot_key) {
                        match ex.get_ticker(symbol).await {
                            Ok(ticker) if ticker.last > 0.0 => Some(ticker.last),
                            _ => None,
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        }
    }
}

// ── Auto PriceProvider ──

pub struct AutoExchangePriceProvider {
    exchange_registry: Arc<Exchanges>,
    kline_engine: Option<Arc<KlineEngine>>,
}

impl AutoExchangePriceProvider {
    pub fn new(exchange_registry: Arc<Exchanges>) -> Self {
        Self {
            exchange_registry,
            kline_engine: None,
        }
    }

    pub fn with_kline_engine(mut self, engine: Arc<KlineEngine>) -> Self {
        self.kline_engine = Some(engine);
        self
    }
}

#[async_trait]
impl PriceProvider for AutoExchangePriceProvider {
    async fn get_price(&self, exchange: &str, symbol: &str, market_type: &str) -> Option<f64> {
        // Try kline engine first
        if let Some(ref engine) = self.kline_engine {
            if let Some(candles) = engine
                .get_klines_async(exchange, symbol, Timeframe::M1)
                .await
            {
                if let Some(last) = candles.last() {
                    if last.close > 0.0 {
                        return Some(last.close);
                    }
                }
            }
        }

        // Fallback to exchange ticker
        let exchange_key = format!("{}:{}", exchange, market_type);
        let ex = self.exchange_registry.get(&exchange_key)?;
        match ex.get_ticker(symbol).await {
            Ok(ticker) if ticker.last > 0.0 => Some(ticker.last),
            _ => None,
        }
    }
}
