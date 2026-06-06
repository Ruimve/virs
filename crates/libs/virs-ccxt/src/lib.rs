//! CCXT-style unified exchange API layer.

pub mod types;
pub mod errors;
pub mod auth;
pub mod adapter;
pub mod ws_types;

use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;

use errors::ExchangeError;
use auth::Signer;

// Re-export key types for convenience
pub use types::{
    Ticker, Kline, OrderBook, Balance, Order,
    MarketInfo, MarketType, Side, OrderType, OrderStatus,
    PlaceOrderParams, ExchangeCapabilities,
    PositionSide, MarginMode, PositionMode,
    FundingRate, FundingHistoryEntry,
};

/// Unified exchange trait — the core abstraction following CCXT's design.
#[async_trait]
pub trait Exchange: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn capabilities(&self) -> &ExchangeCapabilities;

    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker, ExchangeError>;
    async fn fetch_ohlcv(&self, symbol: &str, timeframe: &str, limit: u32, since: Option<i64>) -> Result<Vec<Kline>, ExchangeError>;

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
                if let Some(last) = all_klines.last() {
                    if k.timestamp <= last.timestamp {
                        continue;
                    }
                }
                all_klines.push(k.clone());
            }
            cursor = batch.last().unwrap().timestamp + 1;
            if (batch.len() as u32) < page_limit {
                break;
            }
        }
        Ok(all_klines)
    }

    async fn fetch_order_book(&self, symbol: &str, limit: u32) -> Result<OrderBook, ExchangeError>;
    async fn fetch_balance(&self) -> Result<Vec<Balance>, ExchangeError>;
    async fn fetch_markets(&self) -> Result<Vec<MarketInfo>, ExchangeError>;
    async fn create_order(&self, params: PlaceOrderParams) -> Result<Order, ExchangeError>;
    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<Order, ExchangeError>;
    async fn fetch_order(&self, symbol: &str, order_id: &str) -> Result<Order, ExchangeError>;
    async fn fetch_open_orders(&self, symbol: Option<&str>) -> Result<Vec<Order>, ExchangeError>;
    async fn set_leverage(&self, symbol: &str, leverage: u32, margin_mode: types::MarginMode) -> Result<(), ExchangeError>;
    async fn fetch_positions(&self, symbol: Option<&str>) -> Result<Vec<types::Position>, ExchangeError>;
    async fn get_position_mode(&self) -> Result<types::PositionMode, ExchangeError>;
    async fn fetch_funding_rate(&self, symbol: &str) -> Result<types::FundingRate, ExchangeError>;
    async fn fetch_funding_history(&self, symbol: &str, start_time: i64, end_time: i64) -> Result<Vec<types::FundingHistoryEntry>, ExchangeError>;
    async fn create_listen_key(&self) -> Result<String, ExchangeError>;
    async fn keepalive_listen_key(&self, _listen_key: &str) -> Result<(), ExchangeError> { Ok(()) }
    async fn ping(&self) -> Result<bool, ExchangeError>;
    async fn load_markets(&mut self) -> Result<(), ExchangeError>;
    fn markets(&self) -> &Option<Vec<MarketInfo>>;
}

/// Base HTTP client for exchange REST API calls.
pub struct ExchangeClient {
    client: Client,
    rate_limiter: std::sync::Arc<tokio::sync::Semaphore>,
    base_url: String,
}

impl ExchangeClient {
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
            if let Some(s) = b.as_str() {
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
}

async fn handle_response(resp: reqwest::Response) -> Result<Value, ExchangeError> {
    let status = resp.status();
    let url = resp.url().to_string();
    let text = resp.text().await
        .map_err(|e| ExchangeError::Network(format!("Failed to read response body: {}", e)))?;

    if !status.is_success() {
        if let Ok(json) = serde_json::from_str::<Value>(&text) {
            let msg = extract_error_message(&json);
            return Err(match status.as_u16() {
                401 | 403 => ExchangeError::Authentication(msg),
                429 => ExchangeError::RateLimited(msg),
                400 | 422 => ExchangeError::InvalidRequest(msg),
                _ => ExchangeError::Http { status: status.as_u16(), body: msg },
            });
        }
        return Err(ExchangeError::Http { status: status.as_u16(), body: text });
    }

    serde_json::from_str::<Value>(&text)
        .map_err(|e| ExchangeError::Internal(format!(
            "Failed to parse response from {}: {} (body: {})",
            url, e, &text[..text.len().min(200)]
        )))
}

fn extract_error_message(json: &Value) -> String {
    if let Some(msg) = json.get("msg").and_then(|v| v.as_str()) {
        if let Some(code) = json.get("code") {
            return format!("[{}] {}", code, msg);
        }
        return msg.to_string();
    }
    if let Some(msg) = json.get("retMsg").and_then(|v| v.as_str()) {
        if let Some(code) = json.get("retCode") {
            return format!("[{}] {}", code, msg);
        }
        return msg.to_string();
    }
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

/// Create an exchange instance by name and market type.
pub fn create_exchange(
    id: &str,
    api_key: &str,
    api_secret: &str,
    passphrase: Option<&str>,
    proxy_url: Option<&str>,
    market_type: &types::MarketType,
) -> Result<Box<dyn Exchange>, ExchangeError> {
    match id.to_lowercase().as_str() {
        "binance" => Ok(Box::new(adapter::binance::BinanceExchange::new(
            api_key, api_secret, proxy_url, market_type,
        )?)),
        "okx" => {
            let pass = passphrase
                .ok_or_else(|| ExchangeError::InvalidRequest("OKX requires a passphrase".into()))?;
            Ok(Box::new(adapter::okx::OkxExchange::new(
                api_key, api_secret, pass, proxy_url, market_type,
            )?))
        }
        "bybit" => Ok(Box::new(adapter::bybit::BybitExchange::new(
            api_key, api_secret, proxy_url, market_type,
        )?)),
        _ => Err(ExchangeError::NotSupported(format!(
            "Exchange '{}' is not supported. Supported: binance, okx, bybit", id
        ))),
    }
}

pub fn parse_f64(v: &Value, field: &str) -> Option<f64> {
    v.get(field).and_then(|f| f.as_f64().or_else(|| f.as_str().and_then(|s| s.parse().ok())))
}

pub fn parse_str(v: &Value, field: &str) -> Option<String> {
    v.get(field).and_then(|f| {
        f.as_str().map(String::from)
            .or_else(|| f.as_i64().map(|n| n.to_string()))
            .or_else(|| f.as_f64().map(|n| n.to_string()))
    })
}

pub fn parse_i64(v: &Value, field: &str) -> Option<i64> {
    v.get(field).and_then(|f| f.as_i64().or_else(|| f.as_str().and_then(|s| s.parse().ok())))
}

pub fn parse_u32(v: &Value, field: &str) -> Option<u32> {
    v.get(field)
        .and_then(|f| f.as_u64().or_else(|| f.as_str().and_then(|s| s.parse().ok())))
        .map(|v| v as u32)
}
