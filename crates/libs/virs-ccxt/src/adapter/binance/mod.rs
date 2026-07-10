//! Binance exchange implementation (perpetual futures only).
//!
//! API endpoints are organized by Binance's path prefixes:
//! - `sapi.rs` — /sapi/v1 (Account & funds: balance, apiRestrictions, etc.)
//! - `fapi.rs` — /fapi/v1 (USDT-M Futures: perpetual trading, positions, funding, etc.)
//!
//! Account endpoints (sapi) are shared across market types.

use tracing::info;

pub mod fapi;
pub mod kline_ws;
pub mod user_data_ws;
pub mod orderbook_ws;
pub mod sapi;

use async_trait::async_trait;
use base64::Engine;
use ed25519_dalek::pkcs8::DecodePrivateKey;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::auth::{hmac_sha256_hex, insert_header, SignedRequest, Signer};
use crate::types::*;
use virs_types::WsFeedEvent;
use crate::{Exchange, ExchangeClient};
use virs_error::ExchangeError;

// ============================================================
// Binance Signer (HMAC-SHA256 via query string)
// ============================================================

/// Binance HMAC-SHA256 request signer (legacy, used with HMAC API Keys).
pub struct BinanceSigner {
    api_key: String,
    api_secret: String,
    /// 服务器时间偏移（毫秒），由 sync_time() 校准
    /// T15: 使用 Arc<AtomicI64> 以便 signer clone 后共享同一偏移
    time_offset_ms: Arc<AtomicI64>,
}

impl BinanceSigner {
    pub fn new(api_key: String, api_secret: String) -> Self {
        Self {
            api_key,
            api_secret,
            time_offset_ms: Arc::new(AtomicI64::new(0)),
        }
    }
}

/// recvWindow 常量（毫秒）— 币安默认 5000ms，显式设置以确保一致行为
const RECV_WINDOW: &str = "5000";

/// T1: 定期时间同步间隔（秒）— 每小时重新校准服务器时间偏移
const TIME_SYNC_INTERVAL_SECS: u64 = 3600;

/// T1: 时间偏移告警阈值（毫秒）— 超过此值时发出 warn 日志
const TIME_OFFSET_WARN_THRESHOLD_MS: i64 = 2_000;

impl Signer for BinanceSigner {
    fn set_time_offset(&self, offset_ms: i64) {
        self.time_offset_ms.store(offset_ms, Ordering::Release);
    }

    fn get_time_offset(&self) -> i64 {
        self.time_offset_ms.load(Ordering::Acquire)
    }

    fn sign_get(
        &self,
        _path: &str,
        query_params: &mut Vec<(String, String)>,
    ) -> Result<SignedRequest, ExchangeError> {
        let timestamp = chrono::Utc::now().timestamp_millis()
            + self.time_offset_ms.load(Ordering::Acquire);
        query_params.push(("recvWindow".into(), RECV_WINDOW.into()));
        query_params.push(("timestamp".into(), timestamp.to_string()));

        let query_string = query_params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");

        let signature = hmac_sha256_hex(&self.api_secret, &query_string);
        query_params.push(("signature".into(), signature));

        let mut headers = reqwest::header::HeaderMap::new();
        insert_header(&mut headers, "x-mbx-apikey", &self.api_key)?;

        Ok(SignedRequest {
            headers,
            query_params: query_params.clone(),
            body: None,
        })
    }

    fn sign_post(
        &self,
        _path: &str,
        body: &mut serde_json::Value,
    ) -> Result<SignedRequest, ExchangeError> {
        let timestamp = chrono::Utc::now().timestamp_millis()
            + self.time_offset_ms.load(Ordering::Acquire);
        let timestamp_str = timestamp.to_string();
        let mut query_params = vec![
            ("recvWindow".into(), RECV_WINDOW.into()),
            ("timestamp".into(), timestamp_str.clone()),
        ];

        let form_body = if let Some(obj) = body.as_object() {
            let mut pairs: Vec<(String, String)> = obj
                .iter()
                .map(|(k, v)| {
                    let val = if let Some(s) = v.as_str() {
                        s.to_string()
                    } else {
                        v.to_string()
                    };
                    (k.clone(), val)
                })
                .collect();
            pairs.push(("recvWindow".into(), RECV_WINDOW.into()));
            pairs.push(("timestamp".into(), timestamp_str));

            let query_string = pairs
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");

            let signature = hmac_sha256_hex(&self.api_secret, &query_string);
            pairs.push(("signature".into(), signature));

            query_params = pairs;
            Some(serde_json::Value::String(query_string))
        } else {
            None
        };

        let mut headers = reqwest::header::HeaderMap::new();
        insert_header(&mut headers, "x-mbx-apikey", &self.api_key)?;

        Ok(SignedRequest {
            headers,
            query_params,
            body: form_body,
        })
    }
}

// ============================================================
// Binance Ed25519 Signer (for Ed25519 API Keys — required by WebSocket API)
// ============================================================

/// Binance Ed25519 request signer.
///
/// 用于替代 HMAC-SHA256 的签名方式，币安推荐用于 WebSocket API 的 userDataStream.subscribe。
/// 签名输出为 base64 编码（币安 REST/WebSocket API 的 Ed25519 签名格式）。
///
/// 支持的私钥格式：
/// - PKCS8 PEM（币安后台下载的默认格式）
/// - base64 编码的 32 字节种子
pub struct BinanceEd25519Signer {
    api_key: String,
    signing_key: ed25519_dalek::SigningKey,
    /// 服务器时间偏移（毫秒），由 sync_time() 校准
    /// T15: 使用 Arc<AtomicI64>，clone 后共享同一偏移，确保 sync_time 更新对所有副本生效
    time_offset_ms: Arc<AtomicI64>,
}

impl Clone for BinanceEd25519Signer {
    fn clone(&self) -> Self {
        Self {
            api_key: self.api_key.clone(),
            signing_key: self.signing_key.clone(),
            // T15: 共享同一 Arc<AtomicI64>，而非复制值到新原子
            time_offset_ms: Arc::clone(&self.time_offset_ms),
        }
    }
}

impl BinanceEd25519Signer {
    /// 从 PKCS8 PEM 格式私钥创建
    pub fn from_pem(api_key: &str, pem: &str) -> Result<Self, ExchangeError> {
        let signing_key = ed25519_dalek::SigningKey::from_pkcs8_pem(pem).map_err(|e| {
            ExchangeError::Internal(format!("Invalid Ed25519 PEM private key: {}", e))
        })?;
        Ok(Self {
            api_key: api_key.to_string(),
            signing_key,
            time_offset_ms: Arc::new(AtomicI64::new(0)),
        })
    }

    /// 从 base64 编码的 32 字节种子创建
    pub fn from_seed_b64(api_key: &str, seed_b64: &str) -> Result<Self, ExchangeError> {
        let seed = base64::engine::general_purpose::STANDARD
            .decode(seed_b64.trim())
            .map_err(|e| ExchangeError::Internal(format!("Invalid Ed25519 seed base64: {}", e)))?;
        if seed.len() != 32 {
            return Err(ExchangeError::Internal(format!(
                "Ed25519 seed must be 32 bytes, got {}",
                seed.len()
            )));
        }
        let seed_arr: [u8; 32] = seed.as_slice().try_into().unwrap();
        Ok(Self {
            api_key: api_key.to_string(),
            signing_key: ed25519_dalek::SigningKey::from_bytes(&seed_arr),
            time_offset_ms: Arc::new(AtomicI64::new(0)),
        })
    }

    /// 对消息签名，返回 base64 编码的签名
    pub fn sign_message(&self, message: &str) -> String {
        use ed25519_dalek::Signer;
        let signature = self.signing_key.sign(message.as_bytes());
        base64::engine::general_purpose::STANDARD.encode(signature.to_bytes())
    }

}

impl Signer for BinanceEd25519Signer {
    fn set_time_offset(&self, offset_ms: i64) {
        self.time_offset_ms.store(offset_ms, Ordering::Release);
    }

    fn get_time_offset(&self) -> i64 {
        self.time_offset_ms.load(Ordering::Acquire)
    }

    fn sign_get(
        &self,
        _path: &str,
        query_params: &mut Vec<(String, String)>,
    ) -> Result<SignedRequest, ExchangeError> {
        let timestamp = chrono::Utc::now().timestamp_millis()
            + self.time_offset_ms.load(Ordering::Acquire);
        query_params.push(("recvWindow".into(), RECV_WINDOW.into()));
        query_params.push(("timestamp".into(), timestamp.to_string()));

        let query_string = query_params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");

        let signature = self.sign_message(&query_string);
        query_params.push(("signature".into(), signature));

        let mut headers = reqwest::header::HeaderMap::new();
        insert_header(&mut headers, "x-mbx-apikey", &self.api_key)?;

        Ok(SignedRequest {
            headers,
            query_params: query_params.clone(),
            body: None,
        })
    }

    fn sign_post(
        &self,
        _path: &str,
        body: &mut serde_json::Value,
    ) -> Result<SignedRequest, ExchangeError> {
        let timestamp = chrono::Utc::now().timestamp_millis()
            + self.time_offset_ms.load(Ordering::Acquire);
        let timestamp_str = timestamp.to_string();
        let mut query_params = vec![
            ("recvWindow".into(), RECV_WINDOW.into()),
            ("timestamp".into(), timestamp_str.clone()),
        ];

        let form_body = if let Some(obj) = body.as_object() {
            let mut pairs: Vec<(String, String)> = obj
                .iter()
                .map(|(k, v)| {
                    let val = if let Some(s) = v.as_str() {
                        s.to_string()
                    } else {
                        v.to_string()
                    };
                    (k.clone(), val)
                })
                .collect();
            pairs.push(("recvWindow".into(), RECV_WINDOW.into()));
            pairs.push(("timestamp".into(), timestamp_str));

            let query_string = pairs
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");

            let signature = self.sign_message(&query_string);
            pairs.push(("signature".into(), signature));

            query_params = pairs;
            Some(serde_json::Value::String(query_string))
        } else {
            None
        };

        let mut headers = reqwest::header::HeaderMap::new();
        insert_header(&mut headers, "x-mbx-apikey", &self.api_key)?;

        Ok(SignedRequest {
            headers,
            query_params,
            body: form_body,
        })
    }
}

/// 尝试根据 api_secret 格式构造 Ed25519 签名器：
/// - 以 `-----BEGIN` 开头 → Ed25519 PEM
/// - base64 解码后为 32 字节 → Ed25519 seed
/// - 其他 → 返回 Err（调用方 fallback 到 HMAC）
pub(crate) fn try_build_ed25519(
    api_key: &str,
    api_secret: &str,
) -> Result<BinanceEd25519Signer, ExchangeError> {
    let trimmed = api_secret.trim();
    if trimmed.starts_with("-----BEGIN") {
        BinanceEd25519Signer::from_pem(api_key, trimmed)
    } else if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(trimmed) {
        if decoded.len() == 32 {
            BinanceEd25519Signer::from_seed_b64(api_key, trimmed)
        } else {
            Err(ExchangeError::Internal(format!(
                "api_secret base64 decodes to {} bytes (not 32), treating as HMAC secret",
                decoded.len()
            )))
        }
    } else {
        Err(ExchangeError::Internal(
            "api_secret is not base64, treating as HMAC secret".into(),
        ))
    }
}

// ============================================================
// Binance Exchange
// ============================================================

/// Binance exchange implementation.
///
/// Binance perpetual futures exchange.
///
/// Dispatches to `fapi` module for all market data and trading operations.
/// Account endpoints (`sapi`) are also available.
pub struct BinanceExchange {
    client: ExchangeClient,
    signer: Arc<dyn Signer>,
    /// T1: 标记定期时间同步是否已启动，防止重复 spawn
    time_sync_started: AtomicBool,
    /// T1 WARN fix: 定期时间同步的运行标志，Drop 时设为 false 以停止后台 task
    time_sync_running: Arc<AtomicBool>,
    /// listenKey 保活间隔（秒）— 合约
    listenkey_keepalive_futures_secs: u64,
    /// WS 重连初始延迟（秒）
    ws_reconnect_initial_delay_secs: u64,
    /// WS 重连最大延迟（秒）
    ws_reconnect_max_delay_secs: u64,
    /// WS ping/pong 心跳间隔（秒）
    ws_ping_interval_secs: u64,
    /// WS 连接最大生命周期（秒）
    ws_max_lifetime_secs: u64,
}

impl BinanceExchange {
    /// Create a new Binance exchange instance.
    ///
    /// 根据 `api_secret` 格式自动选择签名器：
    /// - Ed25519 PEM / base64 seed → `BinanceEd25519Signer`（支持 WebSocket API）
    /// - 其他 → `BinanceSigner`（HMAC-SHA256，向后兼容）
    pub fn new(
        api_key: &str,
        api_secret: &str,
        proxy_url: Option<&str>,
        http_timeout: std::time::Duration,
        connect_timeout: std::time::Duration,
        pool_max_idle_per_host: usize,
        listenkey_keepalive_futures_secs: u64,
        ws_reconnect_initial_delay_secs: u64,
        ws_reconnect_max_delay_secs: u64,
        ws_ping_interval_secs: u64,
        ws_max_lifetime_secs: u64,
    ) -> Result<Self, ExchangeError> {
        let max_concurrent: u32 = 40;
        let client =
            ExchangeClient::with_api_key(max_concurrent, proxy_url, Some(api_key), http_timeout, connect_timeout, pool_max_idle_per_host)?;

        let signer = match try_build_ed25519(api_key, api_secret) {
            Ok(ed) => {
                let arc: Arc<dyn Signer> = Arc::new(ed);
                arc
            }
            Err(_) => {
                Arc::new(BinanceSigner::new(
                    api_key.to_string(),
                    api_secret.to_string(),
                )) as Arc<dyn Signer>
            }
        };

        Ok(Self {
            client,
            signer,
            time_sync_started: AtomicBool::new(false),
            time_sync_running: Arc::new(AtomicBool::new(false)),
            listenkey_keepalive_futures_secs,
            ws_reconnect_initial_delay_secs,
            ws_reconnect_max_delay_secs,
            ws_ping_interval_secs,
            ws_max_lifetime_secs,
        })
    }

    /// Convert unified symbol (e.g. "BTC/USDT") to Binance format (e.g. "BTCUSDT").
    pub fn to_native_symbol(symbol: &str) -> String {
        symbol.replace(['/', '-'], "")
    }

    /// Convert Binance symbol to unified format.
    pub fn to_unified_symbol(native: &str) -> String {
        let quotes = [
            "USDT", "USDC", "BUSD", "BTC", "ETH", "BNB", "EUR", "GBP", "TRY", "BRL", "ARS",
        ];
        for q in &quotes {
            if let Some(base) = native.strip_suffix(q) {
                if !base.is_empty() {
                    return format!("{}/{}", base, q);
                }
            }
        }
        native.to_string()
    }

    /// Parse Binance order status string to unified CcxtOrderStatus.
    pub fn parse_order_status(status: &str) -> CcxtOrderStatus {
        match status {
            "NEW" => CcxtOrderStatus::Open,
            "PARTIALLY_FILLED" => CcxtOrderStatus::PartiallyFilled,
            "FILLED" => CcxtOrderStatus::Filled,
            "CANCELED" | "CANCELLED" | "EXPIRED" | "EXPIRED_IN_MATCH" => CcxtOrderStatus::Canceled,
            "REJECTED" => CcxtOrderStatus::Rejected,
            "PENDING_CANCEL" => CcxtOrderStatus::Open,
            _ => CcxtOrderStatus::Open,
        }
    }

    /// Parse Binance order type string to unified OrderType.
    pub fn parse_order_type(order_type: &str) -> OrderType {
        match order_type {
            "MARKET" => OrderType::Market,
            "LIMIT" => OrderType::Limit,
            "STOP_MARKET" => OrderType::StopMarket,
            "STOP" | "STOP_LIMIT" | "TAKE_PROFIT_LIMIT" => OrderType::StopLimit,
            "TAKE_PROFIT_MARKET" | "TAKE_PROFIT" => OrderType::TakeProfitMarket,
            _ => OrderType::Market,
        }
    }

    /// Convert unified Side to Binance string.
    pub fn side_str(side: &Side) -> &'static str {
        match side {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
        }
    }

    /// Convert unified OrderType to Binance futures string.
    ///
    /// 合约订单类型参考: https://developers.binance.com/docs/derivatives/usds-margined-futures/general-info
    /// - StopLimit → `STOP`（合约使用 STOP 表示止损限价单）
    pub fn order_type_str(order_type: &OrderType) -> &'static str {
        match order_type {
            OrderType::Market => "MARKET",
            OrderType::Limit => "LIMIT",
            OrderType::StopMarket => "STOP_MARKET",
            OrderType::StopLimit => "STOP",
            OrderType::TakeProfitMarket => "TAKE_PROFIT_MARKET",
        }
    }
}

// ============================================================
// Shared parsing helpers (used by fapi.rs)
// ============================================================

/// Parse order book bids/asks from exchange response.
///
/// Used by perpetual (`/fapi/v1/depth`) endpoint.
pub(crate) fn parse_order_book_side(data: &serde_json::Value, side: &str) -> Vec<(f64, f64)> {
    data.get(side)
        .and_then(|b| b.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|b| {
                    let a = b.as_array()?;
                    Some((a[0].as_str()?.parse().ok()?, a[1].as_str()?.parse().ok()?))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[async_trait]
impl Exchange for BinanceExchange {
    fn id(&self) -> &str {
        "binance"
    }
    fn name(&self) -> &str {
        "Binance"
    }

    // ---- Market data ----

    async fn fetch_ticker(&self, symbol: &str) -> Result<CcxtTicker, ExchangeError> {
        fapi::fetch_ticker(&self.client, symbol).await
    }

    async fn fetch_ohlcv(
        &self,
        symbol: &str,
        timeframe: &str,
        limit: u32,
        since: Option<i64>,
    ) -> Result<Vec<CcxtKline>, ExchangeError> {
        fapi::fetch_ohlcv(&self.client, symbol, timeframe, limit, since).await
    }

    async fn fetch_order_book(
        &self,
        symbol: &str,
        limit: u32,
    ) -> Result<CcxtOrderBook, ExchangeError> {
        fapi::fetch_order_book(&self.client, symbol, limit).await
    }

    async fn fetch_balance(&self) -> Result<Vec<Balance>, ExchangeError> {
        fapi::fetch_balance(&self.client, self.signer.as_ref()).await
    }

    async fn fetch_markets(&self) -> Result<Vec<MarketInfo>, ExchangeError> {
        fapi::fetch_markets(&self.client).await
    }

    // ---- Trading ----

    async fn create_order(&self, params: PlaceOrderParams) -> Result<CcxtOrder, ExchangeError> {
        fapi::create_order(&self.client, self.signer.as_ref(), params).await
    }

    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<CcxtOrder, ExchangeError> {
        fapi::cancel_order(&self.client, self.signer.as_ref(), symbol, order_id).await
    }

    async fn fetch_order(&self, symbol: &str, order_id: &str) -> Result<CcxtOrder, ExchangeError> {
        fapi::fetch_order(&self.client, self.signer.as_ref(), symbol, order_id).await
    }

    async fn fetch_open_orders(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<CcxtOrder>, ExchangeError> {
        fapi::fetch_open_orders(&self.client, self.signer.as_ref(), symbol).await
    }

    // ---- Perpetual-only ----

    async fn set_leverage(
        &self,
        symbol: &str,
        leverage: u32,
        margin_mode: MarginMode,
    ) -> Result<(), ExchangeError> {
        fapi::set_margin_type(&self.client, self.signer.as_ref(), symbol, margin_mode).await?;
        fapi::set_leverage(&self.client, self.signer.as_ref(), symbol, leverage).await
    }

    async fn fetch_positions(&self, symbol: Option<&str>) -> Result<Vec<Position>, ExchangeError> {
        fapi::fetch_positions(&self.client, self.signer.as_ref(), symbol).await
    }

    async fn get_position_mode(&self) -> Result<PositionMode, ExchangeError> {
        fapi::get_position_mode(&self.client, self.signer.as_ref()).await
    }

    async fn fetch_funding_rate(&self, symbol: &str) -> Result<CcxtFundingRate, ExchangeError> {
        fapi::fetch_funding_rate(&self.client, symbol).await
    }

    async fn fetch_funding_history(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<CcxtFundingHistoryEntry>, ExchangeError> {
        fapi::fetch_funding_history(&self.client, symbol, start_time, end_time).await
    }

    // ---- User data stream ----

    async fn create_listen_key(&self) -> Result<String, ExchangeError> {
        fapi::create_listen_key(&self.client, self.signer.as_ref()).await
    }

    async fn keepalive_listen_key(&self, listen_key: &str) -> Result<(), ExchangeError> {
        fapi::keepalive_listen_key(&self.client, self.signer.as_ref(), listen_key).await
    }

    // ---- Account (sapi) ----

    async fn fetch_api_restrictions(&self) -> Result<ApiRestrictions, ExchangeError> {
        sapi::fetch_api_restrictions(&self.client, self.signer.as_ref()).await
    }

    // ---- listenKey 订单 WS（合约用户数据流）----

    async fn start_listenkey_order_ws(
        &self,
        listen_key_hint: Option<&str>,
    ) -> Result<mpsc::Receiver<WsFeedEvent>, ExchangeError> {
        // 1. 获取 listenKey：优先使用 hint，否则创建
        let listen_key = match listen_key_hint {
            Some(k) => k.to_string(),
            None => self.create_listen_key().await?,
        };

        // 2. 构造合约 UserDataWs
        let mut ws = user_data_ws::UserDataWs::new_perpetual(
            listen_key.clone(),
            self.ws_reconnect_initial_delay_secs,
            self.ws_reconnect_max_delay_secs,
            self.ws_ping_interval_secs,
            self.ws_max_lifetime_secs,
        );

        // 3. 获取 running flag 引用（keepalive task 据此判断 WS 是否已退出）
        let ws_running = ws.running_handle();

        // 4. 启动 WS 并返回 receiver
        let (tx, rx) = mpsc::channel(256);
        ws.start(tx).await;
        info!("listenKey order WS started (perpetual)");

        // 5. spawn listenKey REST keepalive task
        //    币安要求：合约 60 分钟内 keepalive 一次，否则 listenKey 失效。
        //    取保守间隔（窗口的 1/2）以容忍网络抖动。
        let client = self.client.clone();
        let signer = Arc::clone(&self.signer);
        let keepalive_interval = Duration::from_secs(self.listenkey_keepalive_futures_secs);

        tokio::spawn(async move {
            let mut tick = tokio::time::interval(keepalive_interval);
            // 跳过第一次立即触发（WS 刚启动，listenKey 刚创建，无需立即 keepalive）
            tick.tick().await;

            loop {
                if !ws_running.load(Ordering::Relaxed) {
                    return;
                }
                tick.tick().await;
                if !ws_running.load(Ordering::Relaxed) {
                    return;
                }
                let result = fapi::keepalive_listen_key(&client, signer.as_ref(), &listen_key).await;
                match result {
                    Ok(()) => {
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "[BinanceExchange] listenKey keepalive failed, \
                             WS may disconnect when listenKey expires"
                        );
                    }
                }
            }
        });

        Ok(rx)
    }

    // ---- Misc ----

    async fn ping(&self) -> Result<bool, ExchangeError> {
        fapi::ping(&self.client).await
    }

    async fn sync_time(&self) -> Result<(), ExchangeError> {
        let server_time = fapi::fetch_server_time(&self.client).await?;
        let local_time = chrono::Utc::now().timestamp_millis();
        let offset = server_time - local_time;
        self.signer.set_time_offset(offset);

        // T1: Log warning if offset exceeds threshold
        if offset.abs() > TIME_OFFSET_WARN_THRESHOLD_MS {
            tracing::warn!(
                time_offset_ms = offset,
                threshold_ms = TIME_OFFSET_WARN_THRESHOLD_MS,
                "Server time offset exceeds threshold — clock drift detected"
            );
        }

        info!(
            time_offset_ms = offset,
            "Server time synced"
        );

        // T1: Start periodic time sync loop (once per exchange instance)
        if !self.time_sync_started.swap(true, Ordering::SeqCst) {
            self.time_sync_running.store(true, Ordering::Release);
            self.spawn_periodic_time_sync();
        }

        Ok(())
    }
}

impl BinanceExchange {
    /// T1: Spawn a background task that periodically re-syncs server time.
    ///
    /// This prevents clock drift from causing -1021 signature failures
    /// during long-running bot sessions (24/7 operation).
    ///
    /// T1 WARN fix: task checks `time_sync_running` flag and exits when set to false
    /// (in Drop impl), preventing task leak on credential rotation.
    fn spawn_periodic_time_sync(&self) {
        let client = self.client.clone();
        let signer = self.signer.clone();
        let running = self.time_sync_running.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(TIME_SYNC_INTERVAL_SECS));
            // Skip the first immediate tick (initial sync already done)
            interval.tick().await;

            loop {
                // T1 WARN fix: check if exchange has been dropped
                if !running.load(Ordering::Acquire) {
                    tracing::info!("[PeriodicSync] Exchange dropped, stopping time sync task");
                    break;
                }
                interval.tick().await;
                // Re-check after tick (in case exchange was dropped during sleep)
                if !running.load(Ordering::Acquire) {
                    tracing::info!("[PeriodicSync] Exchange dropped during sleep, stopping");
                    break;
                }
                let result = fapi::fetch_server_time(&client).await;
                match result {
                    Ok(server_time) => {
                        let local_time = chrono::Utc::now().timestamp_millis();
                        let offset = server_time - local_time;
                        signer.set_time_offset(offset);

                        if offset.abs() > TIME_OFFSET_WARN_THRESHOLD_MS {
                            tracing::warn!(
                                time_offset_ms = offset,
                                threshold_ms = TIME_OFFSET_WARN_THRESHOLD_MS,
                                "[PeriodicSync] Server time offset exceeds threshold — clock drift detected"
                            );
                        } else {
                            tracing::info!(
                                time_offset_ms = offset,
                                "[PeriodicSync] Server time re-synced successfully"
                            );
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "[PeriodicSync] Failed to re-sync server time — will retry next cycle"
                        );
                    }
                }
            }
        });
    }
}

/// T1 WARN fix: Drop impl stops the periodic time sync task
impl Drop for BinanceExchange {
    fn drop(&mut self) {
        // Signal the periodic sync task to stop
        self.time_sync_running.store(false, Ordering::Release);
    }
}

// ============================================================
// Test modules (_tests suffix pattern)
// ============================================================
#[cfg(test)]
mod kline_ws_tests;
#[cfg(test)]
mod mod_tests;
#[cfg(test)]
mod orderbook_ws_tests;
#[cfg(test)]
mod user_data_ws_tests;
