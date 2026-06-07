//! Price providers — ExchangeRegistry + KlineEngine backed price lookups.

use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;

use virs_exchange::ExchangeRegistry;
use virs_market::KlineEngine;
use virs_market::Timeframe;
use virs_types::bot::PriceProvider;

// ── Grid PriceProvider ──

pub struct ExchangePriceProvider {
    exchange_registry: Arc<ExchangeRegistry>,
    kline_engine: Option<Arc<KlineEngine>>,
    db: Option<PgPool>,
    encryption_key: Option<String>,
}

impl ExchangePriceProvider {
    pub fn new(exchange_registry: Arc<ExchangeRegistry>) -> Self {
        Self { exchange_registry, kline_engine: None, db: None, encryption_key: None }
    }

    pub fn with_kline_engine(mut self, engine: Arc<KlineEngine>) -> Self {
        self.kline_engine = Some(engine);
        self
    }

    pub fn with_db(mut self, db: PgPool, encryption_key: String) -> Self {
        self.db = Some(db);
        self.encryption_key = Some(encryption_key);
        self
    }

    async fn ensure_exchange(&self, exchange: &str, market_type: &str) {
        let exchange_key = format!("{}:{}", exchange, market_type);
        if self.exchange_registry.get(&exchange_key).is_some() { return; }

        let db = match self.db { Some(ref db) => db, None => return };
        let ek = match self.encryption_key { Some(ref ek) => ek, None => return };

        let row: Option<(String, String, Option<String>)> = sqlx::query_as(
            r#"SELECT encrypted_api_key, encrypted_api_secret, encrypted_passphrase
               FROM qd_exchange_credentials
               WHERE exchange = $1 AND market_type = $2 LIMIT 1"#,
        )
        .bind(exchange).bind(market_type)
        .fetch_optional(db).await.unwrap_or(None);

        if let Some((enc_key, enc_secret, enc_passphrase)) = row {
            let derived_key = virs_utils::crypto::derive_key(ek);
            let api_key = match virs_utils::crypto::decrypt(&enc_key, &derived_key) { Ok(k) => k, Err(_) => return };
            let api_secret = match virs_utils::crypto::decrypt(&enc_secret, &derived_key) { Ok(s) => s, Err(_) => return };
            let passphrase = enc_passphrase.and_then(|p| virs_utils::crypto::decrypt(&p, &derived_key).ok());

            let mt = match market_type {
                "spot" => virs_ccxt::MarketType::Spot,
                _ => virs_ccxt::MarketType::Perpetual,
            };

            if let Ok(ccxt_ex) = virs_ccxt::create_exchange(
                exchange, &api_key, &api_secret, passphrase.as_deref(), None, &mt,
            ) {
                let app_mt = match market_type {
                    "spot" => virs_models::MarketType::Spot,
                    _ => virs_models::MarketType::Perpetual,
                };
                let adapter = virs_exchange::CcxtAdapter::new(ccxt_ex, app_mt);
                self.exchange_registry.register(Box::new(adapter));
            }
        }
    }
}

#[async_trait]
impl PriceProvider for ExchangePriceProvider {
    async fn get_price(&self, exchange: &str, symbol: &str, market_type: &str) -> Option<f64> {
        // Try kline engine first (1m candle)
        if let Some(ref engine) = self.kline_engine {
            if let Some(candles) = engine.get_klines_async(exchange, symbol, Timeframe::M1).await {
                if let Some(last) = candles.last() {
                    if last.close > 0.0 {
                        return Some(last.close);
                    }
                }
            }
        }

        // Ensure exchange is registered
        self.ensure_exchange(exchange, market_type).await;

        // Fallback to exchange ticker
        let exchange_key = format!("{}:{}", exchange, market_type);
        let ex = self.exchange_registry.get(&exchange_key)?;
        match ex.get_ticker(symbol).await {
            Ok(ticker) if ticker.last > 0.0 => Some(ticker.last),
            _ => {
                // Also try spot if perpetual failed
                if market_type != "spot" {
                    self.ensure_exchange(exchange, "spot").await;
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
    exchange_registry: Arc<ExchangeRegistry>,
    kline_engine: Option<Arc<KlineEngine>>,
    db: Option<PgPool>,
    encryption_key: Option<String>,
}

impl AutoExchangePriceProvider {
    pub fn new(exchange_registry: Arc<ExchangeRegistry>) -> Self {
        Self { exchange_registry, kline_engine: None, db: None, encryption_key: None }
    }

    pub fn with_kline_engine(mut self, engine: Arc<KlineEngine>) -> Self {
        self.kline_engine = Some(engine);
        self
    }

    pub fn with_db(mut self, db: PgPool, encryption_key: String) -> Self {
        self.db = Some(db);
        self.encryption_key = Some(encryption_key);
        self
    }

    async fn ensure_exchange(&self, exchange: &str, market_type: &str) {
        let exchange_key = format!("{}:{}", exchange, market_type);
        if self.exchange_registry.get(&exchange_key).is_some() { return; }

        let db = match self.db { Some(ref db) => db, None => return };
        let ek = match self.encryption_key { Some(ref ek) => ek, None => return };

        let row: Option<(String, String, Option<String>)> = sqlx::query_as(
            r#"SELECT encrypted_api_key, encrypted_api_secret, encrypted_passphrase
               FROM qd_exchange_credentials
               WHERE exchange = $1 AND market_type = $2 LIMIT 1"#,
        )
        .bind(exchange).bind(market_type)
        .fetch_optional(db).await.unwrap_or(None);

        if let Some((enc_key, enc_secret, enc_passphrase)) = row {
            let derived_key = virs_utils::crypto::derive_key(ek);
            let api_key = match virs_utils::crypto::decrypt(&enc_key, &derived_key) { Ok(k) => k, Err(_) => return };
            let api_secret = match virs_utils::crypto::decrypt(&enc_secret, &derived_key) { Ok(s) => s, Err(_) => return };
            let passphrase = enc_passphrase.and_then(|p| virs_utils::crypto::decrypt(&p, &derived_key).ok());

            let mt = match market_type {
                "spot" => virs_ccxt::MarketType::Spot,
                _ => virs_ccxt::MarketType::Perpetual,
            };

            if let Ok(ccxt_ex) = virs_ccxt::create_exchange(
                exchange, &api_key, &api_secret, passphrase.as_deref(), None, &mt,
            ) {
                let app_mt = match market_type {
                    "spot" => virs_models::MarketType::Spot,
                    _ => virs_models::MarketType::Perpetual,
                };
                let adapter = virs_exchange::CcxtAdapter::new(ccxt_ex, app_mt);
                self.exchange_registry.register(Box::new(adapter));
            }
        }
    }
}

#[async_trait]
impl PriceProvider for AutoExchangePriceProvider {
    async fn get_price(&self, exchange: &str, symbol: &str, market_type: &str) -> Option<f64> {
        // Try kline engine first
        if let Some(ref engine) = self.kline_engine {
            if let Some(candles) = engine.get_klines_async(exchange, symbol, Timeframe::M1).await {
                if let Some(last) = candles.last() {
                    if last.close > 0.0 {
                        return Some(last.close);
                    }
                }
            }
        }

        // Ensure exchange is registered
        self.ensure_exchange(exchange, market_type).await;

        // Fallback to exchange ticker
        let exchange_key = format!("{}:{}", exchange, market_type);
        let ex = self.exchange_registry.get(&exchange_key)?;
        match ex.get_ticker(symbol).await {
            Ok(ticker) if ticker.last > 0.0 => Some(ticker.last),
            _ => None,
        }
    }
}
