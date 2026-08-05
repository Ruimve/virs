pub mod adapter;
pub mod auth;
pub mod types;

use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use tokio::sync::mpsc;

use auth::Signer;
use virs_error::ExchangeError;
use virs_type::{
    ApiRestrictions, Balance, ExchangePosition, MarginMode, OrderResult, PositionMode,
};

pub use types::{
    CcxtFundingRate, CcxtKline, CcxtOrderBook, CcxtPlaceOrderParams, CcxtTicker, MarketInfo,
    OrderFee,
};

#[async_trait]
pub trait Exchange: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;

    async fn fetch_ticker(&self, symbol: &str) -> Result<CcxtTicker, ExchangeError>;
    async fn fetch_ohlcv(
        &self,
        symbol: &str,
        timeframe: &str,
        limit: u32,
        since: Option<i64>,
    ) -> Result<Vec<CcxtKline>, ExchangeError>;

    async fn fetch_ohlcv_range(
        &self,
        symbol: &str,
        timeframe: &str,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<Vec<CcxtKline>, ExchangeError> {
        let page_limit: u32 = 1000;
        let mut all_klines: Vec<CcxtKline> = Vec::new();
        let mut cursor = start_ms;

        while cursor < end_ms {
            let batch = self
                .fetch_ohlcv(symbol, timeframe, page_limit, Some(cursor))
                .await?;
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
            let last = batch.last().ok_or_else(|| {
                ExchangeError::Internal(
                    "batch unexpectedly empty after is_empty check — pagination logic error".into(),
                )
            })?;
            cursor = last.timestamp + 1;
            if (batch.len() as u32) < page_limit {
                break;
            }
        }
        Ok(all_klines)
    }

    async fn fetch_order_book(
        &self,
        symbol: &str,
        limit: u32,
    ) -> Result<CcxtOrderBook, ExchangeError>;
    async fn fetch_balance(&self) -> Result<Vec<Balance>, ExchangeError>;
    async fn fetch_markets(&self) -> Result<Vec<MarketInfo>, ExchangeError>;
    async fn create_order(&self, params: CcxtPlaceOrderParams) -> Result<OrderResult, ExchangeError>;
    async fn cancel_order(
        &self,
        symbol: &str,
        order_id: &str,
    ) -> Result<OrderResult, ExchangeError>;
    async fn cancel_all_orders(&self, symbol: &str) -> Result<(), ExchangeError>;
    async fn set_leverage(
        &self,
        symbol: &str,
        leverage: u32,
        margin_mode: MarginMode,
    ) -> Result<(), ExchangeError>;
    async fn fetch_positions(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<ExchangePosition>, ExchangeError>;
    async fn get_position_mode(&self) -> Result<PositionMode, ExchangeError>;
    async fn fetch_funding_rate(&self, symbol: &str) -> Result<CcxtFundingRate, ExchangeError>;
    async fn create_listen_key(&self) -> Result<String, ExchangeError>;
    async fn fetch_api_restrictions(&self) -> Result<ApiRestrictions, ExchangeError> {
        Err(ExchangeError::NotSupported(
            "fetch_api_restrictions not supported".into(),
        ))
    }

    async fn start_listenkey_order_ws(
        &self,
        _listen_key_hint: Option<&str>,
    ) -> Result<mpsc::Receiver<virs_type::WsFeedEvent>, ExchangeError> {
        Err(ExchangeError::NotSupported(
            "start_listenkey_order_ws not supported".into(),
        ))
    }
    async fn ping(&self) -> Result<bool, ExchangeError>;

    async fn sync_time(&self) -> Result<(), ExchangeError> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct ExchangeClient {
    client: Client,
    rate_limiter: std::sync::Arc<tokio::sync::Semaphore>,

    api_key: Option<String>,
}

impl ExchangeClient {
    pub fn with_api_key(
        max_concurrent: u32,
        proxy_url: Option<&str>,
        api_key: Option<&str>,
        http_timeout: std::time::Duration,
        connect_timeout: std::time::Duration,
        pool_max_idle_per_host: usize,
    ) -> Result<Self, ExchangeError> {
        let mut builder = Client::builder()
            .timeout(http_timeout)
            .connect_timeout(connect_timeout)
            .pool_max_idle_per_host(pool_max_idle_per_host)
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
            api_key: api_key.map(|s| s.to_string()),
        })
    }

    pub async fn public_get(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<Value, ExchangeError> {
        let _permit = self
            .rate_limiter
            .acquire()
            .await
            .map_err(|e| ExchangeError::Internal(format!("Rate limiter error: {}", e)))?;
        let display_url = build_display_url(path, params.iter().map(|(k, v)| (*k, *v)));
        let mut req = self.client.get(path).query(params);

        if let Some(ref key) = self.api_key {
            req = req.header("x-mbx-apikey", key);
        }
        handle_response(req.send().await?, &display_url, None).await
    }

    pub async fn signed_get(
        &self,
        signer: &dyn Signer,
        path: &str,
        mut params: Vec<(String, String)>,
    ) -> Result<Value, ExchangeError> {
        let _permit = self
            .rate_limiter
            .acquire()
            .await
            .map_err(|e| ExchangeError::Internal(format!("Rate limiter error: {}", e)))?;
        let signed = signer.sign_get(path, &mut params)?;
        let display_url = build_display_url(
            path,
            signed
                .query_params
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str())),
        );
        let mut req = self.client.get(path);
        for (k, v) in &signed.query_params {
            req = req.query(&[(k.as_str(), v.as_str())]);
        }
        for (name, value) in signed.headers {
            if let Some(n) = name {
                req = req.header(n, value);
            }
        }
        handle_response(req.send().await?, &display_url, None).await
    }

    pub async fn signed_post(
        &self,
        signer: &dyn Signer,
        path: &str,
        mut body: Value,
    ) -> Result<Value, ExchangeError> {
        let _permit = self
            .rate_limiter
            .acquire()
            .await
            .map_err(|e| ExchangeError::Internal(format!("Rate limiter error: {}", e)))?;
        let signed = signer.sign_post(path, &mut body)?;
        let display_body = signed
            .body
            .as_ref()
            .and_then(|b| b.as_str())
            .map(mask_signature);
        let mut req = self.client.post(path);
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
        handle_response(req.send().await?, path, display_body.as_deref()).await
    }

    pub async fn signed_put(
        &self,
        signer: &dyn Signer,
        path: &str,
        mut body: Value,
    ) -> Result<Value, ExchangeError> {
        let _permit = self
            .rate_limiter
            .acquire()
            .await
            .map_err(|e| ExchangeError::Internal(format!("Rate limiter error: {}", e)))?;
        let signed = signer.sign_put(path, &mut body)?;
        let display_body = signed
            .body
            .as_ref()
            .and_then(|b| b.as_str())
            .map(mask_signature);
        let mut req = self.client.put(path);
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
        handle_response(req.send().await?, path, display_body.as_deref()).await
    }

    pub async fn signed_delete(
        &self,
        signer: &dyn Signer,
        path: &str,
        mut params: Vec<(String, String)>,
    ) -> Result<Value, ExchangeError> {
        let _permit = self
            .rate_limiter
            .acquire()
            .await
            .map_err(|e| ExchangeError::Internal(format!("Rate limiter error: {}", e)))?;
        let signed = signer.sign_get(path, &mut params)?;
        let display_url = build_display_url(
            path,
            signed
                .query_params
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str())),
        );
        let mut req = self.client.delete(path);
        for (k, v) in &signed.query_params {
            req = req.query(&[(k.as_str(), v.as_str())]);
        }
        for (name, value) in signed.headers {
            if let Some(n) = name {
                req = req.header(n, value);
            }
        }
        handle_response(req.send().await?, &display_url, None).await
    }
}

pub(crate) fn build_display_url<'a>(
    path: &str,
    params: impl Iterator<Item = (&'a str, &'a str)>,
) -> String {
    let mut url = path.to_string();
    let mut param_strs: Vec<String> = Vec::new();
    let mut has_params = false;
    for (k, v) in params {
        has_params = true;
        let v = if k == "signature" { "***MASKED***" } else { v };
        param_strs.push(format!("{}={}", k, v));
    }
    if has_params {
        url.push('?');
        url.push_str(&param_strs.join("&"));
    }
    url
}

pub(crate) fn mask_signature(s: &str) -> String {
    if let Some(idx) = s.find("signature=") {
        let before = &s[..idx];
        let after = if let Some(amp_idx) = s[idx..].find('&') {
            &s[idx + amp_idx..]
        } else {
            ""
        };
        format!("{}signature=***MASKED***{}", before, after)
    } else {
        s.to_string()
    }
}

async fn handle_response(
    resp: reqwest::Response,
    display_url: &str,
    _display_body: Option<&str>,
) -> Result<Value, ExchangeError> {
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| ExchangeError::Network(format!("Failed to read response body: {}", e)))?;

    if !status.is_success() {
        if let Ok(json) = serde_json::from_str::<Value>(&text) {
            let msg = extract_error_message(&json);
            return Err(match status.as_u16() {
                401 | 403 => ExchangeError::Authentication(msg),
                418 => ExchangeError::IpBanned(msg),
                429 => ExchangeError::RateLimited(msg),
                400 | 422 => ExchangeError::InvalidRequest(msg),

                503 if msg.contains("Unknown error") => ExchangeError::OrderStatusUnknown(msg),
                _ => ExchangeError::Http {
                    status: status.as_u16(),
                    body: msg,
                },
            });
        }

        if status.as_u16() == 503 && text.contains("Unknown error") {
            return Err(ExchangeError::OrderStatusUnknown(text));
        }
        if status.as_u16() == 418 {
            return Err(ExchangeError::IpBanned(text));
        }
        return Err(ExchangeError::Http {
            status: status.as_u16(),
            body: text,
        });
    }

    serde_json::from_str::<Value>(&text).map_err(|e| {
        let preview: String = text.chars().take(200).collect();
        ExchangeError::Internal(format!(
            "Failed to parse response from {}: {} (body: {})",
            display_url,
            e,
            preview
        ))
    })
}

pub(crate) fn extract_error_message(json: &Value) -> String {
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

pub fn create_exchange(
    id: &str,
    api_key: &str,
    api_secret: &str,
    _passphrase: Option<&str>,
    proxy_url: Option<&str>,
    http_timeout: std::time::Duration,
    connect_timeout: std::time::Duration,
    pool_max_idle_per_host: usize,
    listenkey_keepalive_futures_secs: u64,
) -> Result<Box<dyn Exchange>, ExchangeError> {
    match id.to_lowercase().as_str() {
        "binance" => Ok(Box::new(adapter::binance::BinanceExchange::new(
            api_key,
            api_secret,
            proxy_url,
            http_timeout,
            connect_timeout,
            pool_max_idle_per_host,
            listenkey_keepalive_futures_secs,
        )?)),

        _ => Err(ExchangeError::NotSupported(format!(
            "Exchange '{}' is not supported. Currently implemented: binance. \
             Planned (architecture ready): bybit, okx. \
             See create_exchange documentation for adding new exchanges.",
            id
        ))),
    }
}

pub fn parse_f64(v: &Value, field: &str) -> Option<f64> {
    v.get(field).and_then(|f| {
        f.as_f64()
            .or_else(|| f.as_str().and_then(|s| s.parse().ok()))
    })
}

pub fn parse_str(v: &Value, field: &str) -> Option<String> {
    v.get(field).and_then(|f| {
        f.as_str()
            .map(String::from)
            .or_else(|| f.as_i64().map(|n| n.to_string()))
            .or_else(|| f.as_f64().map(|n| n.to_string()))
    })
}

pub fn parse_u32(v: &Value, field: &str) -> Option<u32> {
    v.get(field)
        .and_then(|f| {
            f.as_u64()
                .or_else(|| f.as_str().and_then(|s| s.parse().ok()))
        })
        .map(|v| v as u32)
}

pub fn parse_timestamp_ms(v: &Value, field: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    v.get(field)
        .and_then(|f| {
            f.as_i64()
                .or_else(|| f.as_str().and_then(|s| s.parse::<i64>().ok()))
        })
        .and_then(chrono::DateTime::from_timestamp_millis)
}

#[cfg(test)]
mod auth_tests;
#[cfg(test)]
mod errors_tests;
#[cfg(test)]
mod lib_tests;
#[cfg(test)]
mod types_tests;
