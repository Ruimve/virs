//! CCXT-style unified exchange API layer.
//!
//! This module implements a CCXT-inspired unified exchange interface, isolated
//! from the rest of the application. Key design principles:
//!
//! 1. **Unified Exchange trait** — all exchanges implement the same interface
//! 2. **Consistent types** — exchange-specific data is normalized to unified types
//! 3. **Per-exchange signers** — each exchange has its own authentication logic
//! 4. **Factory pattern** — create exchange instances by name
//! 5. **No mock data** — if data is unavailable, return explicit errors
//!
//! Architecture:
//! ```text
//! ccxt/
//! ├── mod.rs        (Exchange trait, ExchangeClient base, factory)
//! ├── types.rs      (unified types: Ticker, Kline, Order, etc.)
//! ├── errors.rs     (ExchangeError hierarchy)
//! ├── auth.rs       (Signer trait, Binance/OKX/Bybit signers)
//! ├── binance.rs    (Binance implementation)
//! ├── okx.rs        (OKX implementation)
//! └── bybit.rs      (Bybit implementation)
//! ```

pub mod types;
pub mod errors;
pub mod auth;
pub mod binance;
pub mod okx;
pub mod bybit;

use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;

use errors::ExchangeError;
use auth::Signer;

// Re-export key types for convenience
pub use types::{
    Ticker, Kline, OrderBook, Balance, Order,
    MarketInfo, MarketType, Side, OrderType, OrderStatus,
    PlaceOrderParams, ExchangeCapabilities, FundingHistoryEntry,
};

/// Unified exchange trait — the core abstraction following CCXT's design.
///
/// All exchange implementations must implement this trait. Methods return
/// `Result<T, ExchangeError>` — never mock data.
#[async_trait]
pub trait Exchange: Send + Sync {
    /// Return the exchange identifier (e.g., "binance", "okx").
    fn id(&self) -> &str;

    /// Return exchange display name.
    fn name(&self) -> &str;

    /// Return exchange capabilities.
    fn capabilities(&self) -> &ExchangeCapabilities;

    // ---- Market Data (Public) ----

    /// Fetch a single ticker.
    /// Returns `ExchangeError::NoData` if the symbol is not found.
    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker, ExchangeError>;

    /// Fetch OHLCV (candlestick) data.
    /// Returns `ExchangeError::NoData` if no candles are available.
    async fn fetch_ohlcv(
        &self,
        symbol: &str,
        timeframe: &str,
        limit: u32,
        since: Option<i64>,
    ) -> Result<Vec<Kline>, ExchangeError>;

    /// Fetch OHLCV data for a full time range [start_ms, end_ms] with pagination.
    /// Default implementation pages through `fetch_ohlcv` calls.
    /// Each exchange can override this for optimal pagination.
    async fn fetch_ohlcv_range(
        &self,
        symbol: &str,
        timeframe: &str,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<Vec<Kline>, ExchangeError> {
        let page_limit: u32 = 1000;
        let mut all_klines: Vec<Kline> = Vec::new();
        let mut cursor = start_ms;

        while cursor < end_ms {
            let batch = self.fetch_ohlcv(symbol, timeframe, page_limit, Some(cursor)).await?;
            if batch.is_empty() {
                break;
            }

            for k in &batch {
                if k.timestamp > end_ms {
                    return Ok(all_klines);
                }
                // Avoid duplicates
                if let Some(last) = all_klines.last() {
                    if k.timestamp <= last.timestamp {
                        continue;
                    }
                }
                all_klines.push(k.clone());
            }

            // Move cursor past the last timestamp
            cursor = batch.last().unwrap().timestamp + 1;

            // If we got fewer than page_limit, we've reached the end
            if (batch.len() as u32) < page_limit {
                break;
            }
        }

        Ok(all_klines)
    }

    /// Fetch the order book.
    /// Returns `ExchangeError::NoData` if the symbol is not found.
    async fn fetch_order_book(
        &self,
        symbol: &str,
        limit: u32,
    ) -> Result<OrderBook, ExchangeError>;

    /// Fetch account balances.
    /// Returns `ExchangeError::Authentication` if credentials are invalid.
    async fn fetch_balance(&self) -> Result<Vec<Balance>, ExchangeError>;

    /// Fetch all available markets with trading rules.
    async fn fetch_markets(&self) -> Result<Vec<MarketInfo>, ExchangeError>;

    // ---- Trading (Authenticated) ----

    /// Place a new order.
    async fn create_order(&self, params: PlaceOrderParams) -> Result<Order, ExchangeError>;

    /// Cancel an existing order.
    async fn cancel_order(
        &self,
        symbol: &str,
        order_id: &str,
    ) -> Result<Order, ExchangeError>;

    /// Fetch a single order by ID.
    async fn fetch_order(
        &self,
        symbol: &str,
        order_id: &str,
    ) -> Result<Order, ExchangeError>;

    /// Fetch all open orders.
    async fn fetch_open_orders(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<Order>, ExchangeError>;

    // ---- Perpetual Contracts (Authenticated) ----

    /// Set leverage for a perpetual contract.
    /// Returns `ExchangeError::NotSupported` for spot exchanges.
    async fn set_leverage(
        &self,
        symbol: &str,
        leverage: u32,
        margin_mode: types::MarginMode,
    ) -> Result<(), ExchangeError>;

    /// Fetch current positions.
    /// Returns `ExchangeError::NotSupported` for spot exchanges.
    async fn fetch_positions(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<types::Position>, ExchangeError>;

    /// Fetch funding rate for a perpetual contract.
    /// Returns `ExchangeError::NotSupported` for spot exchanges.
    async fn fetch_funding_rate(
        &self,
        symbol: &str,
    ) -> Result<types::FundingRate, ExchangeError>;

    /// Fetch historical funding rates for a perpetual contract within a time range.
    /// Returns `ExchangeError::NotSupported` for spot exchanges.
    async fn fetch_funding_history(
        &self,
        symbol: &str,
        start_time: i64,  // Unix timestamp in milliseconds
        end_time: i64,    // Unix timestamp in milliseconds
    ) -> Result<Vec<types::FundingHistoryEntry>, ExchangeError>;

    // ---- System ----

    /// Check if the exchange is reachable.
    async fn ping(&self) -> Result<bool, ExchangeError>;

    /// Load markets cache (called once at startup).
    async fn load_markets(&mut self) -> Result<(), ExchangeError>;

    /// Get cached markets (after load_markets).
    fn markets(&self) -> &Option<Vec<MarketInfo>>;
}

/// Base HTTP client for exchange REST API calls.
/// Provides rate limiting, proxy support, and common request methods.
pub struct ExchangeClient {
    client: Client,
    rate_limiter: std::sync::Arc<tokio::sync::Semaphore>,
    base_url: String,
}

impl ExchangeClient {
    /// Create a new exchange HTTP client.
    pub fn new(base_url: &str, max_concurrent: u32, proxy_url: Option<&str>) -> Result<Self, ExchangeError> {
        let mut builder = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .pool_max_idle_per_host(10)
            .gzip(true);

        if let Some(proxy) = proxy_url {
            let proxy = reqwest::Proxy::all(proxy)
                .map_err(|e| ExchangeError::Internal(format!("Invalid proxy config: {}", e)))?;
            builder = builder.proxy(proxy);
        }

        let client = builder
            .build()
            .map_err(|e| ExchangeError::Internal(format!("Failed to build HTTP client: {}", e)))?;

        Ok(Self {
            client,
            rate_limiter: std::sync::Arc::new(tokio::sync::Semaphore::new(max_concurrent as usize)),
            base_url: base_url.to_string(),
        })
    }

    /// Get the base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Make a public GET request (no authentication).
    pub async fn public_get(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<Value, ExchangeError> {
        let _permit = self.rate_limiter.acquire().await
            .map_err(|e| ExchangeError::Internal(format!("Rate limiter error: {}", e)))?;

        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.get(&url).query(params).send().await?;

        handle_response(resp).await
    }

    /// Make an authenticated GET request.
    pub async fn signed_get(
        &self,
        signer: &dyn Signer,
        path: &str,
        mut params: Vec<(String, String)>,
    ) -> Result<Value, ExchangeError> {
        let _permit = self.rate_limiter.acquire().await
            .map_err(|e| ExchangeError::Internal(format!("Rate limiter error: {}", e)))?;

        let signed = signer.sign_get(path, &mut params)?;

        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.get(&url);
        for (k, v) in &signed.query_params {
            req = req.query(&[(k.as_str(), v.as_str())]);
        }
        for (name, value) in signed.headers {
            if let Some(n) = name {
                req = req.header(n, value);
            }
        }

        handle_response(req.send().await?).await
    }

    /// Make an authenticated POST request (JSON body).
    pub async fn signed_post(
        &self,
        signer: &dyn Signer,
        path: &str,
        mut body: Value,
    ) -> Result<Value, ExchangeError> {
        let _permit = self.rate_limiter.acquire().await
            .map_err(|e| ExchangeError::Internal(format!("Rate limiter error: {}", e)))?;

        let signed = signer.sign_post(path, &mut body)?;

        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.post(&url);
        for (name, value) in signed.headers {
            if let Some(n) = name {
                req = req.header(n, value);
            }
        }

        if let Some(b) = signed.body {
            // Check if it's a form string or JSON
            if let Some(s) = b.as_str() {
                // Form-urlencoded body
                req = req
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(s.to_string());
            } else {
                req = req.json(&b);
            }
        } else {
            req = req.json(&body);
        }

        handle_response(req.send().await?).await
    }

    /// Make a public POST request (no auth, JSON body).
    pub async fn public_post(
        &self,
        path: &str,
        body: &Value,
    ) -> Result<Value, ExchangeError> {
        let _permit = self.rate_limiter.acquire().await
            .map_err(|e| ExchangeError::Internal(format!("Rate limiter error: {}", e)))?;

        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.post(&url).json(body).send().await?;

        handle_response(resp).await
    }
}

/// Handle HTTP response — check status, parse JSON, detect exchange errors.
async fn handle_response(resp: reqwest::Response) -> Result<Value, ExchangeError> {
    let status = resp.status();
    let url = resp.url().to_string();

    // Try to parse body as text first for error messages
    let text = resp.text().await
        .map_err(|e| ExchangeError::Network(format!("Failed to read response body: {}", e)))?;

    if !status.is_success() {
        // Try to extract error message from JSON body
        if let Ok(json) = serde_json::from_str::<Value>(&text) {
            let msg = extract_error_message(&json);
            return Err(match status.as_u16() {
                401 | 403 => ExchangeError::Authentication(msg),
                429 => ExchangeError::RateLimited(msg),
                400 | 422 => ExchangeError::InvalidRequest(msg),
                _ => ExchangeError::Http {
                    status: status.as_u16(),
                    body: msg,
                },
            });
        }
        return Err(ExchangeError::Http {
            status: status.as_u16(),
            body: text,
        });
    }

    // Parse successful response
    serde_json::from_str::<Value>(&text)
        .map_err(|e| ExchangeError::Internal(format!(
            "Failed to parse response from {}: {} (body: {})",
            url, e, &text[..text.len().min(200)]
        )))
}

/// Extract error message from various exchange response formats.
fn extract_error_message(json: &Value) -> String {
    // Binance: { "code": -1021, "msg": "Timestamp for this request..." }
    if let Some(msg) = json.get("msg").and_then(|v| v.as_str()) {
        if let Some(code) = json.get("code") {
            return format!("[{}] {}", code, msg);
        }
        return msg.to_string();
    }

    // OKX: { "code": "51000", "msg": "..." }
    if let Some(msg) = json.get("msg").and_then(|v| v.as_str()) {
        if let Some(code) = json.get("code") {
            return format!("[{}] {}", code, msg);
        }
        return msg.to_string();
    }

    // Bybit: { "retCode": 10001, "retMsg": "..." }
    if let Some(msg) = json.get("retMsg").and_then(|v| v.as_str()) {
        if let Some(code) = json.get("retCode") {
            return format!("[{}] {}", code, msg);
        }
        return msg.to_string();
    }

    // Generic: { "error": "...", "message": "...", "detail": "..." }
    if let Some(err) = json.get("error").and_then(|v| v.as_str()) {
        return err.to_string();
    }
    if let Some(msg) = json.get("message").and_then(|v| v.as_str()) {
        return msg.to_string();
    }
    if let Some(detail) = json.get("detail").and_then(|v| v.as_str()) {
        return detail.to_string();
    }

    json.to_string()
}

// ============================================================
// Factory
// ============================================================

/// Create an exchange instance by name and market type.
///
/// This is the main entry point for creating exchange connections.
/// Returns an error if the exchange name is not supported.
pub fn create_exchange(
    id: &str,
    api_key: &str,
    api_secret: &str,
    passphrase: Option<&str>,
    proxy_url: Option<&str>,
    market_type: &types::MarketType,
) -> Result<Box<dyn Exchange>, ExchangeError> {
    match id.to_lowercase().as_str() {
        "binance" => Ok(Box::new(binance::BinanceExchange::new(
            api_key, api_secret, proxy_url, market_type,
        )?)),
        "okx" => {
            let pass = passphrase
                .ok_or_else(|| ExchangeError::InvalidRequest(
                    "OKX requires a passphrase".into()
                ))?;
            Ok(Box::new(okx::OkxExchange::new(
                api_key, api_secret, pass, proxy_url, market_type,
            )?))
        }
        "bybit" => Ok(Box::new(bybit::BybitExchange::new(
            api_key, api_secret, proxy_url, market_type,
        )?)),
        _ => Err(ExchangeError::NotSupported(format!(
            "Exchange '{}' is not supported. Supported: binance, okx, bybit",
            id
        ))),
    }
}

// ============================================================
// Helper functions for parsing exchange responses
// ============================================================

/// Parse a string or number field as f64.
pub fn parse_f64(v: &Value, field: &str) -> Option<f64> {
    v.get(field).and_then(|f| {
        f.as_f64()
            .or_else(|| f.as_str().and_then(|s| s.parse().ok()))
    })
}

/// Parse a string field.
pub fn parse_str(v: &Value, field: &str) -> String {
    v.get(field)
        .and_then(|f| {
            f.as_str()
                .map(String::from)
                .or_else(|| f.as_i64().map(|n| n.to_string()))
                .or_else(|| f.as_f64().map(|n| n.to_string()))
        })
        .unwrap_or_default()
}

pub fn parse_str_opt(v: &Value, field: &str) -> Option<String> {
    v.get(field).and_then(|f| {
        f.as_str()
            .map(String::from)
            .or_else(|| f.as_i64().map(|n| n.to_string()))
            .or_else(|| f.as_f64().map(|n| n.to_string()))
    })
}

/// Parse a numeric field as i64.
pub fn parse_i64(v: &Value, field: &str) -> i64 {
    v.get(field)
        .and_then(|f| f.as_i64().or_else(|| f.as_str().and_then(|s| s.parse().ok())))
        .unwrap_or(0)
}

/// Parse a numeric field as u32.
pub fn parse_u32(v: &Value, field: &str) -> u32 {
    v.get(field)
        .and_then(|f| f.as_u64().or_else(|| f.as_str().and_then(|s| s.parse().ok())))
        .unwrap_or(0) as u32
}
