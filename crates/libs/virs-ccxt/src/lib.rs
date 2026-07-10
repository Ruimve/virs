//! CCXT-style unified exchange API layer.

pub mod adapter;
pub mod auth;
pub mod types;
pub mod ws_types;

use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use tokio::sync::mpsc;

use auth::Signer;
use virs_error::ExchangeError;

// Re-export shared types from virs-types (via types module)
pub use types::{
    ApiRestrictions,
    Balance,
    CcxtFundingHistoryEntry,
    CcxtFundingRate,
    CcxtKline,
    // CCXT-internal types
    CcxtOrder,
    CcxtOrderBook,
    CcxtOrderStatus,
    CcxtTicker,
    FundingHistoryEntry,
    FundingRate,
    Kline,
    MarginMode,
    MarketInfo,
    MarketType,
    OrderBook,
    OrderFee,
    OrderStatus,
    OrderType,
    PlaceOrderParams,
    PositionMode,
    PositionSide,
    Side,
    // Shared types (re-exported from virs-types)
    Ticker,
};

/// Unified exchange trait — the core abstraction following CCXT's design.
///
/// Methods return CCXT-internal types (CcxtTicker, CcxtOrder, etc.) which
/// carry raw exchange data (`info` field). The CcxtAdapter in virs-exchange
/// converts these to application-level types via `From` impls.
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
    async fn create_order(&self, params: PlaceOrderParams) -> Result<CcxtOrder, ExchangeError>;
    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<CcxtOrder, ExchangeError>;
    async fn fetch_order(&self, symbol: &str, order_id: &str) -> Result<CcxtOrder, ExchangeError>;
    async fn fetch_open_orders(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<CcxtOrder>, ExchangeError>;
    async fn set_leverage(
        &self,
        symbol: &str,
        leverage: u32,
        margin_mode: MarginMode,
    ) -> Result<(), ExchangeError>;
    async fn fetch_positions(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<types::Position>, ExchangeError>;
    async fn get_position_mode(&self) -> Result<PositionMode, ExchangeError>;
    async fn fetch_funding_rate(&self, symbol: &str) -> Result<CcxtFundingRate, ExchangeError>;
    async fn fetch_funding_history(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<CcxtFundingHistoryEntry>, ExchangeError>;
    async fn create_listen_key(&self) -> Result<String, ExchangeError>;
    async fn keepalive_listen_key(&self, _listen_key: &str) -> Result<(), ExchangeError> {
        Ok(())
    }
    async fn fetch_api_restrictions(&self) -> Result<types::ApiRestrictions, ExchangeError> {
        Err(ExchangeError::NotSupported(
            "fetch_api_restrictions not supported".into(),
        ))
    }

    /// 启动基于 listenKey 的订单 WebSocket（合约用户数据流）。
    ///
    /// 实现负责：
    /// 1. 调用 `create_listen_key` 获取 listenKey（若调用方未提供）
    /// 2. 构造并启动对应交易所的 UserDataWs
    /// 3. 返回事件 receiver
    ///
    /// 调用方可通过 `listen_key_hint` 传入已缓存的 listenKey 以避免重复创建。
    /// 不支持的交易所返回 `ExchangeError::NotSupported`。
    async fn start_listenkey_order_ws(
        &self,
        _listen_key_hint: Option<&str>,
    ) -> Result<mpsc::Receiver<virs_types::WsFeedEvent>, ExchangeError> {
        Err(ExchangeError::NotSupported(
            "start_listenkey_order_ws not supported".into(),
        ))
    }
    async fn ping(&self) -> Result<bool, ExchangeError>;

    /// 同步本地时钟与交易所服务器时间。
    ///
    /// 获取交易所服务器时间并计算偏移，通过 `Signer::set_time_offset` 校准签名时间戳。
    /// 应在创建交易所实例后、首次签名请求前调用。
    /// 失败时不应阻塞初始化 — 默认偏移 0 + recvWindow 5000ms 可容忍小漂移。
    async fn sync_time(&self) -> Result<(), ExchangeError> {
        Ok(())
    }
}

/// Base HTTP client for exchange REST API calls.
///
/// `Clone` 是廉价操作（内部全为 `Arc`），允许在后台 keepalive task 中持有副本。
#[derive(Clone)]
pub struct ExchangeClient {
    client: Client,
    rate_limiter: std::sync::Arc<tokio::sync::Semaphore>,
    /// 可选 API Key — 用于 MARKET_DATA 接口的 X-MBX-APIKEY 头
    /// （币安鉴权类型表：MARKET_DATA = 需要有效的API-KEY）
    api_key: Option<String>,
}

impl ExchangeClient {
    /// 创建带 API Key 的 client，用于在 public_get 中附加 X-MBX-APIKEY 头。
    ///
    /// `http_timeout` — HTTP 请求总超时，从 TimeConfig.http_timeout_secs 注入。
    /// `connect_timeout` — TCP 连接建立超时，从 TimeConfig.http_connect_timeout_secs 注入。
    /// `pool_max_idle_per_host` — 每主机最大空闲连接数，从 TimeConfig.http_pool_max_idle_per_host 注入。
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
        // MARKET_DATA 接口需要有效的 API-KEY（仅头，无需签名）
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

    /// Signed DELETE request.
    ///
    /// Binance DELETE endpoints (e.g. cancel order) sign parameters via query
    /// string, identical to GET. We reuse `sign_get` for the signature and
    /// issue the request with the DELETE HTTP method.
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

/// Build a display URL from path and query params, masking `signature` for safe logging.
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

/// Mask `signature=XXX` in a URL-encoded body string for safe logging.
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
                // 503 + "Unknown error" = 执行状态未知（可能已成交），不得盲目重试
                503 if msg.contains("Unknown error") => ExchangeError::OrderStatusUnknown(msg),
                _ => ExchangeError::Http {
                    status: status.as_u16(),
                    body: msg,
                },
            });
        }
        // 非 JSON 响应体也需检测 503 "Unknown error" 与 418
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
        ExchangeError::Internal(format!(
            "Failed to parse response from {}: {} (body: {})",
            display_url,
            e,
            &text[..text.len().min(200)]
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

/// Create an exchange instance by name and market type.
///
/// # Adding a New Exchange
///
/// To add support for a new exchange (e.g., Bybit, OKX), follow these steps:
///
/// 1. Create a new module under `adapter/` (e.g., `adapter/bybit/mod.rs`)
/// 2. Implement the `Exchange` trait for your exchange struct
/// 3. Add the module to `adapter/mod.rs`
/// 4. Add a match arm in this function
///
/// The adapter pattern ensures each exchange implementation is fully isolated.
/// All exchange-specific logic (REST signing, WS parsing, error mapping) lives
/// in its own module, sharing only the unified `Exchange` trait interface.
///
/// # Current Support
///
/// | Exchange | Status |
/// |----------|--------|
/// | Binance  | Implemented (perpetual futures) |
/// | Bybit    | Planned — architecture ready for quick implementation |
/// | OKX      | Planned — architecture ready for quick implementation |
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
    ws_reconnect_initial_delay_secs: u64,
    ws_reconnect_max_delay_secs: u64,
    ws_ping_interval_secs: u64,
    ws_max_lifetime_secs: u64,
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
            ws_reconnect_initial_delay_secs,
            ws_reconnect_max_delay_secs,
            ws_ping_interval_secs,
            ws_max_lifetime_secs,
        )?)),
        // To add Bybit/OKX, implement adapter::bybit::BybitExchange / adapter::okx::OkxExchange
        // and add the corresponding match arm here. See "Adding a New Exchange" above.
        // "bybit" => Ok(Box::new(adapter::bybit::BybitExchange::new(...)?)),
        // "okx" => Ok(Box::new(adapter::okx::OkxExchange::new(...)?)),
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

/// Parse a millisecond timestamp field from a JSON value into `DateTime<Utc>`.
///
/// Binance returns timestamps as integer milliseconds (e.g. `"time": 1568879328023`).
/// This helper is used to extract `created_at`/`updated_at` from order responses
/// instead of using local clock time (F11 fix).
pub fn parse_timestamp_ms(v: &Value, field: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    v.get(field)
        .and_then(|f| {
            f.as_i64()
                .or_else(|| f.as_str().and_then(|s| s.parse::<i64>().ok()))
        })
        .and_then(chrono::DateTime::from_timestamp_millis)
}

// ============================================================
// Test modules (_tests suffix pattern)
// ============================================================
#[cfg(test)]
mod auth_tests;
#[cfg(test)]
mod errors_tests;
#[cfg(test)]
mod lib_tests;
#[cfg(test)]
mod types_tests;
