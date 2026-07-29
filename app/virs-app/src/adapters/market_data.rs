use std::sync::Arc;

use async_trait::async_trait;
use tracing::warn;

use virs_exchange::Exchanges;
use virs_market::KlineEngine;
use virs_market::Timeframe;
use virs_types::Kline;
use virs_error::{VirsError, VirsResult};
use virs_types::bot::{MarketDataProvider, MarketSnapshot};
use virs_types::market::Balance;
use virs_types::exchange_pe::ExchangePe;

pub fn candle_to_kline(c: &virs_ccxt::ws_types::Candle) -> Kline {
    Kline {
        open_time: c.open_time,
        open: c.open,
        high: c.high,
        low: c.low,
        close: c.close,
        volume: c.volume,
        close_time: c.close_time,
        quote_volume: c.quote_volume,
        trades: c.trades,
        symbol: String::new(),
        exchange: String::new(),
        interval: String::new(),
    }
}

pub struct ExchangeMarketDataProvider {
    exchange_registry: Arc<Exchanges>,
    kline_engine: Option<Arc<KlineEngine>>,
    pe_exchange: Option<Arc<dyn ExchangePe>>,
}

impl ExchangeMarketDataProvider {
    pub fn new(exchange_registry: Arc<Exchanges>) -> Self {
        Self {
            exchange_registry,
            kline_engine: None,
            pe_exchange: None,
        }
    }

    pub fn with_kline_engine(mut self, engine: Arc<KlineEngine>) -> Self {
        self.kline_engine = Some(engine);
        self
    }

    pub fn with_pe_exchange(mut self, pe_exchange: Arc<dyn ExchangePe>) -> Self {
        self.pe_exchange = Some(pe_exchange);
        self
    }

    async fn fetch_klines(
        &self,
        exchange: &str,
        symbol: &str,
        timeframe: Timeframe,
        min_count: usize,
        interval_str: &str,
        start_ms: i64,
        required: bool,
    ) -> Option<Vec<Kline>> {
        if let Some(ref engine) = self.kline_engine {
            if let Some(candles) = engine.get_klines_async(exchange, symbol, timeframe).await {
                if candles.len() >= min_count {
                    return Some(candles.iter().map(candle_to_kline).collect());
                }
            }
        }

        let exchange_key = format!("{}:perpetual", exchange);
        let ex = self.exchange_registry.get(&exchange_key);
        match ex {
            Some(ref ex) => match ex
                .get_klines_range(
                    symbol,
                    interval_str,
                    start_ms,
                    chrono::Utc::now().timestamp_millis(),
                )
                .await
            {
                Ok(k) if k.len() >= min_count => Some(k),
                Ok(k) if !required => Some(k),
                Ok(k) => {
                    warn!(
                        exchange,
                        symbol,
                        count = k.len(),
                        required = min_count,
                        "{} klines insufficient",
                        interval_str
                    );
                    if required {
                        None
                    } else {
                        Some(k)
                    }
                }
                Err(e) => {
                    warn!(exchange, symbol, error = %e, "Failed to fetch {} klines", interval_str);
                    if required {
                        None
                    } else {
                        Some(vec![])
                    }
                }
            },
            None => {
                warn!(exchange, symbol, "No exchange for {} klines", interval_str);
                if required {
                    None
                } else {
                    Some(vec![])
                }
            }
        }
    }

    async fn fetch_current_price(&self, exchange: &str, symbol: &str, klines_1h: &[Kline]) -> VirsResult<f64> {
        if let Some(ref engine) = self.kline_engine {
            if let Some(candles) = engine
                .get_klines_async(exchange, symbol, Timeframe::M1)
                .await
            {
                if let Some(last) = candles.last() {
                    if last.close > 0.0 {
                        return Ok(last.close);
                    }
                }
            }
        }

        let exchange_key = format!("{}:perpetual", exchange);
        if let Some(ex) = self.exchange_registry.get(&exchange_key) {
            if let Ok(t) = ex.get_ticker(symbol).await {
                if t.last > 0.0 {
                    return Ok(t.last);
                }
            }
        }

        klines_1h.last().map(|k| k.close).ok_or_else(|| {
            VirsError::Exchange(virs_error::ExchangeError::no_data(format!(
                "All price sources failed for {} on {} — refusing to return 0.0 as price",
                symbol, exchange
            )))
        })
    }
}

#[async_trait]
impl MarketDataProvider for ExchangeMarketDataProvider {
    async fn get_market_snapshot(&self, exchange: &str, symbol: &str) -> VirsResult<MarketSnapshot> {
        let now_ms = chrono::Utc::now().timestamp_millis();

        let klines_1h = match self
            .fetch_klines(
                exchange,
                symbol,
                Timeframe::H1,
                30,
                "1h",
                now_ms - 200 * 3600 * 1000,
                true,
            )
            .await
        {
            Some(k) => k,
            None => return Err(VirsError::Exchange(virs_error::ExchangeError::no_data(format!(
                "No H1 kline data available for {} on {} — cannot build market snapshot",
                symbol, exchange
            )))),
        };

        let klines_4h = self
            .fetch_klines(
                exchange,
                symbol,
                Timeframe::H4,
                50,
                "4h",
                now_ms - 100 * 4 * 3600 * 1000,
                false,
            )
            .await
            .unwrap_or_default();

        let klines_15m = self
            .fetch_klines(
                exchange,
                symbol,
                Timeframe::M15,
                50,
                "15m",
                now_ms - 200 * 15 * 60 * 1000,
                false,
            )
            .await
            .unwrap_or_default();

        let current_price = self.fetch_current_price(exchange, symbol, &klines_1h).await?;

        let exchange_key = format!("{}:perpetual", exchange);
        let funding_next_time;
        let funding_rate = if let Some(ex) = self.exchange_registry.get(&exchange_key) {
                match ex.get_funding_rate(symbol).await {
                    Ok(fr) => {
                        funding_next_time = fr
                            .next_funding_time
                            .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                            .unwrap_or_else(|| "N/A".to_string());
                        fr.rate
                    }
                    Err(e) => {
                        warn!(
                            exchange = %exchange,
                            symbol = %symbol,
                            error = %e,
                            "Funding rate fetch failed"
                        );
                        return Err(VirsError::Exchange(virs_error::ExchangeError::no_data(format!(
                            "Funding rate fetch failed for {} on {}: {}",
                            symbol, exchange, e
                        ))));
                    }
                }
            } else {
                return Err(VirsError::Exchange(virs_error::ExchangeError::no_data(format!(
                    "No exchange found for funding rate: {}",
                    exchange
                ))));
            };

        let ind = virs_strategy::market::compute_market_indicators(
            &klines_1h,
            &klines_4h,
            &klines_15m,
            funding_rate,
            funding_next_time.clone(),
        )?;

        let effective_price = if current_price > 0.0 {
            current_price
        } else {
            ind.current_price
        };

        let exchange_key = format!("{}:perpetual", exchange);
        let min_qty = if let Some(ex) = self.exchange_registry.get(&exchange_key) {
            ex.get_min_qty(symbol).await.map_err(|e| {
                VirsError::Exchange(virs_error::ExchangeError::no_data(format!(
                    "Failed to fetch min_qty for {} on {}: {}",
                    symbol, exchange, e
                )))
            })?
        } else {
            return Err(VirsError::Exchange(virs_error::ExchangeError::no_data(format!(
                "No exchange found for min_qty: {}",
                exchange
            ))));
        };

        Ok(MarketSnapshot {
            current_price: effective_price,
            funding_rate,
            funding_next_time,
            min_qty,
            indicators_json: serde_json::to_value(&ind)
                .map_err(|e| VirsError::config(format!("Failed to serialize indicators: {}", e)))?,
        })
    }

    async fn get_account_balance(&self, exchange: &str) -> VirsResult<Balance> {
        if let Some(ref pe_ex) = self.pe_exchange {
            match pe_ex.get_balance().await {
                Ok(b) => return Ok(b),
                Err(e) => {
                    warn!(error = %e, "PE exchange get_balance failed, falling back to registry");
                }
            }
        }

        let exchange_key = format!("{}:perpetual", exchange);
        let ex = self.exchange_registry.get(&exchange_key)
            .ok_or_else(|| VirsError::Exchange(virs_error::ExchangeError::no_data(format!(
                "No exchange found for balance: {}", exchange
            ))))?;

        ex.get_balance().await.map_err(|e| {
            warn!(error = %e, "get_account_balance error");
            VirsError::Exchange(virs_error::ExchangeError::no_data(format!(
                "Failed to fetch balance for {}: {}", exchange, e
            )))
        })
    }
}

pub struct AutoExchangeMarketDataProvider {
    exchange_registry: Arc<Exchanges>,
    kline_engine: Option<Arc<KlineEngine>>,
    pe_exchange: Option<Arc<dyn ExchangePe>>,
}

impl AutoExchangeMarketDataProvider {
    pub fn new(exchange_registry: Arc<Exchanges>) -> Self {
        Self {
            exchange_registry,
            kline_engine: None,
            pe_exchange: None,
        }
    }

    pub fn with_kline_engine(mut self, engine: Arc<KlineEngine>) -> Self {
        self.kline_engine = Some(engine);
        self
    }

    pub fn with_pe_exchange(mut self, pe_exchange: Arc<dyn ExchangePe>) -> Self {
        self.pe_exchange = Some(pe_exchange);
        self
    }

    async fn fetch_klines(
        &self,
        exchange: &str,
        symbol: &str,
        timeframe: Timeframe,
        min_count: usize,
        interval_str: &str,
        start_ms: i64,
        required: bool,
    ) -> Option<Vec<Kline>> {
        if let Some(ref engine) = self.kline_engine {
            if let Some(candles) = engine.get_klines_async(exchange, symbol, timeframe).await {
                if candles.len() >= min_count {
                    return Some(candles.iter().map(candle_to_kline).collect());
                }
            }
        }

        let exchange_key = format!("{}:perpetual", exchange);
        let ex = self.exchange_registry.get(&exchange_key);
        match ex {
            Some(ref ex) => match ex
                .get_klines_range(
                    symbol,
                    interval_str,
                    start_ms,
                    chrono::Utc::now().timestamp_millis(),
                )
                .await
            {
                Ok(k) if k.len() >= min_count => Some(k),
                Ok(k) if !required => Some(k),
                Ok(k) => {
                    warn!(
                        exchange,
                        symbol,
                        count = k.len(),
                        required = min_count,
                        "{} klines insufficient",
                        interval_str
                    );
                    if required {
                        None
                    } else {
                        Some(k)
                    }
                }
                Err(e) => {
                    warn!(exchange, symbol, error = %e, "Failed to fetch {} klines", interval_str);
                    if required {
                        None
                    } else {
                        Some(vec![])
                    }
                }
            },
            None => {
                warn!(exchange, symbol, "No exchange for {} klines", interval_str);
                if required {
                    None
                } else {
                    Some(vec![])
                }
            }
        }
    }

    async fn fetch_current_price(&self, exchange: &str, symbol: &str, klines_1h: &[Kline]) -> VirsResult<f64> {
        if let Some(ref engine) = self.kline_engine {
            if let Some(candles) = engine
                .get_klines_async(exchange, symbol, Timeframe::M1)
                .await
            {
                if let Some(last) = candles.last() {
                    if last.close > 0.0 {
                        return Ok(last.close);
                    }
                }
            }
        }

        let exchange_key = format!("{}:perpetual", exchange);
        if let Some(ex) = self.exchange_registry.get(&exchange_key) {
            if let Ok(t) = ex.get_ticker(symbol).await {
                if t.last > 0.0 {
                    return Ok(t.last);
                }
            }
        }

        klines_1h.last().map(|k| k.close).ok_or_else(|| {
            VirsError::Exchange(virs_error::ExchangeError::no_data(format!(
                "All price sources failed for {} on {} — refusing to return 0.0 as price",
                symbol, exchange
            )))
        })
    }
}

#[async_trait]
impl MarketDataProvider for AutoExchangeMarketDataProvider {
    async fn get_market_snapshot(&self, exchange: &str, symbol: &str) -> VirsResult<MarketSnapshot> {
        let now_ms = chrono::Utc::now().timestamp_millis();

        let klines_1h = match self
            .fetch_klines(
                exchange,
                symbol,
                Timeframe::H1,
                30,
                "1h",
                now_ms - 200 * 3600 * 1000,
                true,
            )
            .await
        {
            Some(k) => k,
            None => return Err(VirsError::Exchange(virs_error::ExchangeError::no_data(format!(
                "No H1 kline data available for {} on {} — cannot build market snapshot",
                symbol, exchange
            )))),
        };

        let klines_4h = self
            .fetch_klines(
                exchange,
                symbol,
                Timeframe::H4,
                50,
                "4h",
                now_ms - 100 * 4 * 3600 * 1000,
                false,
            )
            .await
            .unwrap_or_default();

        let klines_15m = self
            .fetch_klines(
                exchange,
                symbol,
                Timeframe::M15,
                50,
                "15m",
                now_ms - 200 * 15 * 60 * 1000,
                false,
            )
            .await
            .unwrap_or_default();

        let current_price = self.fetch_current_price(exchange, symbol, &klines_1h).await?;

        let exchange_key = format!("{}:perpetual", exchange);
        let (funding_rate, funding_next_time) =
            if let Some(ex) = self.exchange_registry.get(&exchange_key) {
                match ex.get_funding_rate(symbol).await {
                    Ok(fr) => {
                        let next = fr
                            .next_funding_time
                            .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                            .unwrap_or_else(|| "N/A".to_string());
                        (fr.rate, next)
                    }
                    Err(e) => {
                        return Err(VirsError::Exchange(virs_error::ExchangeError::no_data(format!(
                            "Funding rate fetch failed for {} on {}: {}",
                            symbol, exchange, e
                        ))));
                    }
                }
            } else {
                return Err(VirsError::Exchange(virs_error::ExchangeError::no_data(format!(
                    "No exchange found for funding rate: {}",
                    exchange
                ))));
            };

        let ind = virs_strategy::market::compute_market_indicators(
            &klines_1h,
            &klines_4h,
            &klines_15m,
            funding_rate,
            funding_next_time.clone(),
        )?;

        let effective_price = if current_price > 0.0 {
            current_price
        } else {
            ind.current_price
        };

        let exchange_key = format!("{}:perpetual", exchange);
        let min_qty = if let Some(ex) = self.exchange_registry.get(&exchange_key) {
            ex.get_min_qty(symbol).await.map_err(|e| {
                VirsError::Exchange(virs_error::ExchangeError::no_data(format!(
                    "Failed to fetch min_qty for {} on {}: {}",
                    symbol, exchange, e
                )))
            })?
        } else {
            return Err(VirsError::Exchange(virs_error::ExchangeError::no_data(format!(
                "No exchange found for min_qty: {}",
                exchange
            ))));
        };

        Ok(MarketSnapshot {
            current_price: effective_price,
            funding_rate,
            funding_next_time,
            min_qty,
            indicators_json: serde_json::to_value(&ind)
                .map_err(|e| VirsError::config(format!("Failed to serialize indicators: {}", e)))?,
        })
    }

    async fn get_account_balance(&self, exchange: &str) -> VirsResult<Balance> {
        if let Some(ref pe_ex) = self.pe_exchange {
            match pe_ex.get_balance().await {
                Ok(b) => return Ok(b),
                Err(e) => {
                    warn!(error = %e, "PE exchange get_balance failed, falling back to registry");
                }
            }
        }

        let exchange_key = format!("{}:perpetual", exchange);
        let ex = self.exchange_registry.get(&exchange_key)
            .ok_or_else(|| VirsError::Exchange(virs_error::ExchangeError::no_data(format!(
                "No exchange found for balance: {}", exchange
            ))))?;

        ex.get_balance().await.map_err(|e| {
            warn!(error = %e, "get_account_balance error");
            VirsError::Exchange(virs_error::ExchangeError::no_data(format!(
                "Failed to fetch balance for {}: {}", exchange, e
            )))
        })
    }
}
