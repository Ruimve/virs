//! Market data providers — kline fetching, indicator computation, balance queries.

use std::sync::Arc;

use async_trait::async_trait;
use tracing::{debug, warn};

use virs_exchange::Exchanges;
use virs_market::KlineEngine;
use virs_market::Timeframe;
use virs_models::Kline;
use virs_types::bot::{AccountBalance, MarketDataProvider, MarketSnapshot};
use virs_types::exchange_pe::ExchangePe;

/// Convert a virs-market Candle to virs-models Kline.
pub fn candle_to_kline(c: &virs_market::Candle) -> Kline {
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

// ── Grid MarketDataProvider ──

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
        // Try kline engine cache first
        if let Some(ref engine) = self.kline_engine {
            if let Some(candles) = engine.get_klines_async(exchange, symbol, timeframe).await {
                if candles.len() >= min_count {
                    return Some(candles.iter().map(candle_to_kline).collect());
                }
                debug!(
                    exchange,
                    symbol,
                    timeframe = timeframe.as_str(),
                    cached = candles.len(),
                    required = min_count,
                    "KlineEngine cache insufficient, falling back to REST"
                );
            }
        }

        // Fallback to REST API
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

    async fn fetch_current_price(&self, exchange: &str, symbol: &str, klines_1h: &[Kline]) -> f64 {
        if let Some(ref engine) = self.kline_engine {
            if let Some(candles) = engine
                .get_klines_async(exchange, symbol, Timeframe::M1)
                .await
            {
                if let Some(last) = candles.last() {
                    if last.close > 0.0 {
                        return last.close;
                    }
                }
            }
        }

        let exchange_key = format!("{}:perpetual", exchange);
        if let Some(ex) = self.exchange_registry.get(&exchange_key) {
            if let Ok(t) = ex.get_ticker(symbol).await {
                if t.last > 0.0 {
                    return t.last;
                }
            }
        }

        klines_1h.last().map(|k| k.close).unwrap_or(0.0)
    }
}

#[async_trait]
impl MarketDataProvider for ExchangeMarketDataProvider {
    async fn get_market_snapshot(
        &self,
        exchange: &str,
        symbol: &str,
        market_type: &str,
    ) -> MarketSnapshot {
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
            None => return MarketSnapshot::default(),
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

        let current_price = self.fetch_current_price(exchange, symbol, &klines_1h).await;

        let exchange_key = format!("{}:{}", exchange, market_type);
        let funding_rate = if market_type == "perpetual" {
            if let Some(ex) = self.exchange_registry.get(&exchange_key) {
                ex.get_funding_rate(symbol)
                    .await
                    .map(|fr| fr.rate)
                    .unwrap_or(0.0)
            } else {
                0.0
            }
        } else {
            0.0
        };

        let ind = virs_bot::common::indicators::compute_market_indicators(
            &klines_1h,
            &klines_4h,
            &klines_15m,
            funding_rate,
            "N/A".to_string(),
        );

        let effective_price = if current_price > 0.0 {
            current_price
        } else {
            ind.current_price
        };

        MarketSnapshot {
            current_price: effective_price,
            funding_rate,
            funding_next_time: "N/A".to_string(),
            min_qty: 0.0,
            liquidation_price: None,
            indicators_json: serde_json::to_value(&ind).unwrap_or_default(),
        }
    }

    async fn get_account_balance(&self, exchange: &str, _market_type: &str) -> AccountBalance {
        // Paper mode: use PE exchange for simulated balance
        if let Some(ref pe_ex) = self.pe_exchange {
            match pe_ex.get_balance().await {
                Ok(b) => {
                    return AccountBalance {
                        total: b.total,
                        free: b.free,
                        used: b.used,
                    };
                }
                Err(e) => {
                    warn!(error = %e, "PE exchange get_balance failed, falling back to registry");
                }
            }
        }

        // Real mode: use Exchanges
        let exchange_key = format!("{}:perpetual", exchange);
        let ex = match self.exchange_registry.get(&exchange_key) {
            Some(e) => e,
            None => return AccountBalance::default(),
        };

        match ex.get_balances().await {
            Ok(bs) => {
                let usdt = bs.iter().find(|b| b.asset.eq_ignore_ascii_case("USDT"));
                match usdt {
                    Some(b) => AccountBalance {
                        total: b.total,
                        free: b.free,
                        used: b.used,
                    },
                    None => AccountBalance::default(),
                }
            }
            Err(e) => {
                warn!(error = %e, "get_account_balance error");
                AccountBalance::default()
            }
        }
    }
}

// ── Auto MarketDataProvider ──

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
        market_type: &str,
        required: bool,
    ) -> Option<Vec<Kline>> {
        // Try kline engine cache first
        if let Some(ref engine) = self.kline_engine {
            if let Some(candles) = engine.get_klines_async(exchange, symbol, timeframe).await {
                if candles.len() >= min_count {
                    return Some(candles.iter().map(candle_to_kline).collect());
                }
                debug!(
                    exchange,
                    symbol,
                    timeframe = timeframe.as_str(),
                    cached = candles.len(),
                    required = min_count,
                    "KlineEngine cache insufficient"
                );
            }
        }

        // Fallback to REST API
        let exchange_key = format!("{}:{}", exchange, market_type);
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

    async fn fetch_current_price(
        &self,
        exchange: &str,
        symbol: &str,
        market_type: &str,
        klines_1h: &[Kline],
    ) -> f64 {
        if let Some(ref engine) = self.kline_engine {
            if let Some(candles) = engine
                .get_klines_async(exchange, symbol, Timeframe::M1)
                .await
            {
                if let Some(last) = candles.last() {
                    if last.close > 0.0 {
                        return last.close;
                    }
                }
            }
        }

        let exchange_key = format!("{}:{}", exchange, market_type);
        if let Some(ex) = self.exchange_registry.get(&exchange_key) {
            if let Ok(t) = ex.get_ticker(symbol).await {
                if t.last > 0.0 {
                    return t.last;
                }
            }
        }

        klines_1h.last().map(|k| k.close).unwrap_or(0.0)
    }
}

#[async_trait]
impl MarketDataProvider for AutoExchangeMarketDataProvider {
    async fn get_market_snapshot(
        &self,
        exchange: &str,
        symbol: &str,
        market_type: &str,
    ) -> MarketSnapshot {
        let now_ms = chrono::Utc::now().timestamp_millis();

        let klines_1h = match self
            .fetch_klines(
                exchange,
                symbol,
                Timeframe::H1,
                30,
                "1h",
                now_ms - 200 * 3600 * 1000,
                market_type,
                true,
            )
            .await
        {
            Some(k) => k,
            None => return MarketSnapshot::default(),
        };

        let klines_4h = self
            .fetch_klines(
                exchange,
                symbol,
                Timeframe::H4,
                50,
                "4h",
                now_ms - 100 * 4 * 3600 * 1000,
                market_type,
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
                market_type,
                false,
            )
            .await
            .unwrap_or_default();

        let current_price = self
            .fetch_current_price(exchange, symbol, market_type, &klines_1h)
            .await;

        let exchange_key = format!("{}:{}", exchange, market_type);
        let (funding_rate, funding_next_time) = if market_type == "perpetual" {
            if let Some(ex) = self.exchange_registry.get(&exchange_key) {
                match ex.get_funding_rate(symbol).await {
                    Ok(fr) => {
                        let next = fr
                            .next_funding_time
                            .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                            .unwrap_or_else(|| "N/A".to_string());
                        (fr.rate, next)
                    }
                    Err(_) => (0.0, "N/A".to_string()),
                }
            } else {
                (0.0, "N/A".to_string())
            }
        } else {
            (0.0, "N/A".to_string())
        };

        let ind = virs_bot::common::indicators::compute_market_indicators(
            &klines_1h,
            &klines_4h,
            &klines_15m,
            funding_rate,
            funding_next_time.clone(),
        );

        let effective_price = if current_price > 0.0 {
            current_price
        } else {
            ind.current_price
        };

        // Get min qty
        let min_qty = if let Some(ex) = self.exchange_registry.get(&exchange_key) {
            match ex.get_min_qty(symbol).await {
                Ok(qty) => qty,
                Err(_) => 0.0,
            }
        } else {
            0.0
        };

        // Get liquidation price (perpetual only, when has position)
        let liquidation_price = if market_type == "perpetual" {
            if let Some(ex) = self.exchange_registry.get(&exchange_key) {
                match ex.get_positions(Some(symbol)).await {
                    Ok(positions) => positions
                        .iter()
                        .find(|p| p.symbol.as_str() == symbol && p.size.abs() > 0.0)
                        .and_then(|p| p.liquidation_price),
                    Err(_) => None,
                }
            } else {
                None
            }
        } else {
            None
        };

        MarketSnapshot {
            current_price: effective_price,
            funding_rate,
            funding_next_time,
            min_qty,
            liquidation_price,
            indicators_json: serde_json::to_value(&ind).unwrap_or_default(),
        }
    }

    async fn get_account_balance(&self, exchange: &str, market_type: &str) -> AccountBalance {
        // Paper mode: use PE exchange
        if let Some(ref pe_ex) = self.pe_exchange {
            match pe_ex.get_balance().await {
                Ok(b) => {
                    return AccountBalance {
                        total: b.total,
                        free: b.free,
                        used: b.used,
                    };
                }
                Err(e) => {
                    warn!(error = %e, "PE exchange get_balance failed, falling back to registry");
                }
            }
        }

        // Real mode
        let exchange_key = format!("{}:{}", exchange, market_type);
        let ex = match self.exchange_registry.get(&exchange_key) {
            Some(e) => e,
            None => return AccountBalance::default(),
        };

        match ex.get_balances().await {
            Ok(bs) => {
                let usdt = bs.iter().find(|b| b.asset.eq_ignore_ascii_case("USDT"));
                match usdt {
                    Some(b) => AccountBalance {
                        total: b.total,
                        free: b.free,
                        used: b.used,
                    },
                    None => AccountBalance::default(),
                }
            }
            Err(e) => {
                warn!(error = %e, "get_account_balance error");
                AccountBalance::default()
            }
        }
    }
}
