//! Binance exchange implementation.
//!
//! API endpoints are organized by Binance's path prefixes:
//! - `api.rs`  — /api/v3  (Spot market: ticker, klines, orders, etc.)
//! - `sapi.rs` — /sapi/v1 (Account & funds: balance, apiRestrictions, etc.)
//! - `fapi.rs` — /fapi/v1 (USDT-M Futures: perpetual trading, positions, funding, etc.)
//!
//! The `BinanceExchange` struct dispatches to the appropriate module
//! based on `market_type` (Spot → api, Perpetual → fapi).
//! Account endpoints (sapi) are shared across market types.

pub mod api;
pub mod fapi;
pub mod kline_ws;
pub mod user_data_ws;
pub mod orderbook_ws;
pub mod sapi;
pub mod user_data_ws_api;

use async_trait::async_trait;
use base64::Engine;
use ed25519_dalek::pkcs8::DecodePrivateKey;
use std::sync::atomic::Ordering;
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
}

impl BinanceSigner {
    pub fn new(api_key: String, api_secret: String) -> Self {
        Self {
            api_key,
            api_secret,
        }
    }
}

impl Signer for BinanceSigner {
    fn sign_get(
        &self,
        _path: &str,
        query_params: &mut Vec<(String, String)>,
    ) -> Result<SignedRequest, ExchangeError> {
        let timestamp = chrono::Utc::now().timestamp_millis();
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
        let mut query_params = vec![(
            "timestamp".into(),
            chrono::Utc::now().timestamp_millis().to_string(),
        )];

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
            pairs.push(("timestamp".into(), query_params[0].1.clone()));

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
#[derive(Clone)]
pub struct BinanceEd25519Signer {
    api_key: String,
    signing_key: ed25519_dalek::SigningKey,
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
        })
    }

    /// 对消息签名，返回 base64 编码的签名
    pub fn sign_message(&self, message: &str) -> String {
        use ed25519_dalek::Signer;
        let signature = self.signing_key.sign(message.as_bytes());
        base64::engine::general_purpose::STANDARD.encode(signature.to_bytes())
    }

    /// 返回 API Key
    pub fn api_key(&self) -> &str {
        &self.api_key
    }
}

impl Signer for BinanceEd25519Signer {
    fn sign_get(
        &self,
        _path: &str,
        query_params: &mut Vec<(String, String)>,
    ) -> Result<SignedRequest, ExchangeError> {
        let timestamp = chrono::Utc::now().timestamp_millis();
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
        let mut query_params = vec![(
            "timestamp".into(),
            chrono::Utc::now().timestamp_millis().to_string(),
        )];

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
            pairs.push(("timestamp".into(), query_params[0].1.clone()));

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
        tracing::info!("Detected Ed25519 PEM private key, using BinanceEd25519Signer");
        BinanceEd25519Signer::from_pem(api_key, trimmed)
    } else if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(trimmed) {
        if decoded.len() == 32 {
            tracing::info!("Detected Ed25519 32-byte seed (base64), using BinanceEd25519Signer");
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
/// Dispatches to `api` (spot) or `fapi` (perpetual) modules based on `market_type`.
/// Account endpoints (`sapi`) are available regardless of market type.
pub struct BinanceExchange {
    client: ExchangeClient,
    signer: Arc<dyn Signer>,
    /// 若配置了 Ed25519 签名器，保存引用以便 WebSocket API 客户端使用
    ed25519_signer: Option<BinanceEd25519Signer>,
    #[allow(dead_code)]
    testnet: bool,
    market_type: MarketType,
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
        market_type: &MarketType,
    ) -> Result<Self, ExchangeError> {
        let max_concurrent: u32 = match market_type {
            MarketType::Spot => 20,
            MarketType::Perpetual => 40,
        };
        let client = ExchangeClient::new(max_concurrent, proxy_url)?;

        // 尝试构造 Ed25519 签名器；若不是 Ed25519 格式则 fallback 到 HMAC
        let (signer, ed25519_signer) = match try_build_ed25519(api_key, api_secret) {
            Ok(ed) => {
                let arc: Arc<dyn Signer> = Arc::new(ed.clone());
                (arc, Some(ed))
            }
            Err(_) => (
                Arc::new(BinanceSigner::new(
                    api_key.to_string(),
                    api_secret.to_string(),
                )) as Arc<dyn Signer>,
                None,
            ),
        };

        Ok(Self {
            client,
            signer,
            ed25519_signer,
            testnet: false,
            market_type: *market_type,
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
            "STOP_MARKET" | "STOP_LOSS" => OrderType::StopMarket,
            "STOP_LIMIT" | "STOP_LOSS_LIMIT" | "TAKE_PROFIT_LIMIT" => OrderType::StopLimit,
            "TAKE_PROFIT_MARKET" => OrderType::TakeProfitMarket,
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

    /// Convert unified OrderType to Binance spot string.
    ///
    /// 现货订单类型参考: https://developers.binance.com/docs/binance-spot-api-docs/rest-api
    /// - StopLimit → `STOP_LOSS_LIMIT`（现货不支持统一的 STOP，必须区分止损/止盈，
    ///   统一枚集中 StopLimit 默认映射为止损限价单 STOP_LOSS_LIMIT）
    pub fn order_type_str(order_type: &OrderType) -> &'static str {
        match order_type {
            OrderType::Market => "MARKET",
            OrderType::Limit => "LIMIT",
            OrderType::StopMarket => "STOP_MARKET",
            OrderType::StopLimit => "STOP_LOSS_LIMIT",
            OrderType::TakeProfitMarket => "TAKE_PROFIT_MARKET",
        }
    }

    /// Convert unified OrderType to Binance futures string.
    ///
    /// 合约订单类型参考: https://developers.binance.com/docs/derivatives/usds-margined-futures/general-info
    /// - StopLimit → `STOP`（合约使用 STOP 表示止损限价单，
    ///   与现货的 STOP_LOSS_LIMIT 不同）
    pub fn order_type_str_futures(order_type: &OrderType) -> &'static str {
        match order_type {
            OrderType::Market => "MARKET",
            OrderType::Limit => "LIMIT",
            OrderType::StopMarket => "STOP_MARKET",
            OrderType::StopLimit => "STOP",
            OrderType::TakeProfitMarket => "TAKE_PROFIT_MARKET",
        }
    }

    /// Check if this instance is configured for perpetual futures.
    fn is_perpetual(&self) -> bool {
        self.market_type == MarketType::Perpetual
    }
}

// ============================================================
// Shared parsing helpers (used by api.rs and fapi.rs)
// ============================================================

/// Parse order book bids/asks from exchange response.
///
/// Shared between spot (`/api/v3/depth`) and perpetual (`/fapi/v1/depth`) endpoints.
/// Both return the same JSON structure for bids/asks arrays.
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

    // ---- Market data (dispatch by market_type) ----

    async fn fetch_ticker(&self, symbol: &str) -> Result<CcxtTicker, ExchangeError> {
        if self.is_perpetual() {
            fapi::fetch_ticker(&self.client, symbol).await
        } else {
            api::fetch_ticker(&self.client, symbol).await
        }
    }

    async fn fetch_ohlcv(
        &self,
        symbol: &str,
        timeframe: &str,
        limit: u32,
        since: Option<i64>,
    ) -> Result<Vec<CcxtKline>, ExchangeError> {
        if self.is_perpetual() {
            fapi::fetch_ohlcv(&self.client, symbol, timeframe, limit, since).await
        } else {
            api::fetch_ohlcv(&self.client, symbol, timeframe, limit, since).await
        }
    }

    async fn fetch_order_book(
        &self,
        symbol: &str,
        limit: u32,
    ) -> Result<CcxtOrderBook, ExchangeError> {
        if self.is_perpetual() {
            fapi::fetch_order_book(&self.client, symbol, limit).await
        } else {
            api::fetch_order_book(&self.client, symbol, limit).await
        }
    }

    async fn fetch_balance(&self) -> Result<Vec<Balance>, ExchangeError> {
        if self.is_perpetual() {
            fapi::fetch_balance(&self.client, self.signer.as_ref()).await
        } else {
            api::fetch_balance(&self.client, self.signer.as_ref()).await
        }
    }

    async fn fetch_markets(&self) -> Result<Vec<MarketInfo>, ExchangeError> {
        if self.is_perpetual() {
            fapi::fetch_markets(&self.client).await
        } else {
            api::fetch_markets(&self.client).await
        }
    }

    // ---- Trading (dispatch by market_type) ----

    async fn create_order(&self, params: PlaceOrderParams) -> Result<CcxtOrder, ExchangeError> {
        if self.is_perpetual() {
            fapi::create_order(&self.client, self.signer.as_ref(), params).await
        } else {
            api::create_order(&self.client, self.signer.as_ref(), params).await
        }
    }

    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<CcxtOrder, ExchangeError> {
        if self.is_perpetual() {
            fapi::cancel_order(&self.client, self.signer.as_ref(), symbol, order_id).await
        } else {
            api::cancel_order(&self.client, self.signer.as_ref(), symbol, order_id).await
        }
    }

    async fn fetch_order(&self, symbol: &str, order_id: &str) -> Result<CcxtOrder, ExchangeError> {
        if self.is_perpetual() {
            fapi::fetch_order(&self.client, self.signer.as_ref(), symbol, order_id).await
        } else {
            api::fetch_order(&self.client, self.signer.as_ref(), symbol, order_id).await
        }
    }

    async fn fetch_open_orders(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<CcxtOrder>, ExchangeError> {
        if self.is_perpetual() {
            fapi::fetch_open_orders(&self.client, self.signer.as_ref(), symbol).await
        } else {
            api::fetch_open_orders(&self.client, self.signer.as_ref(), symbol).await
        }
    }

    // ---- Perpetual-only ----

    async fn set_leverage(
        &self,
        symbol: &str,
        leverage: u32,
        margin_mode: MarginMode,
    ) -> Result<(), ExchangeError> {
        if !self.is_perpetual() {
            return Err(ExchangeError::NotSupported(
                "Leverage is only supported for perpetual futures".into(),
            ));
        }
        fapi::set_margin_type(&self.client, self.signer.as_ref(), symbol, margin_mode).await?;
        fapi::set_leverage(&self.client, self.signer.as_ref(), symbol, leverage).await
    }

    async fn fetch_positions(&self, symbol: Option<&str>) -> Result<Vec<Position>, ExchangeError> {
        if !self.is_perpetual() {
            return Err(ExchangeError::NotSupported(
                "Positions are only supported for perpetual futures".into(),
            ));
        }
        fapi::fetch_positions(&self.client, self.signer.as_ref(), symbol).await
    }

    async fn get_position_mode(&self) -> Result<PositionMode, ExchangeError> {
        if !self.is_perpetual() {
            return Err(ExchangeError::NotSupported(
                "Position mode is only supported for perpetual futures".into(),
            ));
        }
        fapi::get_position_mode(&self.client, self.signer.as_ref()).await
    }

    async fn fetch_funding_rate(&self, symbol: &str) -> Result<CcxtFundingRate, ExchangeError> {
        if !self.is_perpetual() {
            return Err(ExchangeError::NotSupported(
                "Funding rate is only supported for perpetual futures".into(),
            ));
        }
        fapi::fetch_funding_rate(&self.client, symbol).await
    }

    async fn fetch_funding_history(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<CcxtFundingHistoryEntry>, ExchangeError> {
        if !self.is_perpetual() {
            return Err(ExchangeError::NotSupported(
                "Funding history is only supported for perpetual futures".into(),
            ));
        }
        fapi::fetch_funding_history(&self.client, symbol, start_time, end_time).await
    }

    // ---- User data stream ----

    async fn create_listen_key(&self) -> Result<String, ExchangeError> {
        if self.is_perpetual() {
            fapi::create_listen_key(&self.client, self.signer.as_ref()).await
        } else {
            api::create_listen_key(&self.client, self.signer.as_ref()).await
        }
    }

    async fn keepalive_listen_key(&self, listen_key: &str) -> Result<(), ExchangeError> {
        if self.is_perpetual() {
            fapi::keepalive_listen_key(&self.client, self.signer.as_ref(), listen_key).await
        } else {
            api::keepalive_listen_key(&self.client, self.signer.as_ref(), listen_key).await
        }
    }

    // ---- Account (sapi) ----

    async fn fetch_api_restrictions(&self) -> Result<ApiRestrictions, ExchangeError> {
        sapi::fetch_api_restrictions(&self.client, self.signer.as_ref()).await
    }

    // ---- WebSocket API (现货 Ed25519 用户数据流) ----

    async fn start_spot_order_ws_api(&self) -> Result<mpsc::Receiver<WsFeedEvent>, ExchangeError> {
        if self.is_perpetual() {
            return Err(ExchangeError::NotSupported(
                "start_spot_order_ws_api is only for spot market".into(),
            ));
        }
        let ed25519 = self.ed25519_signer.as_ref().ok_or_else(|| {
            ExchangeError::NotSupported(
                "WebSocket API requires Ed25519 API Key; \
                 current key is HMAC-SHA256, migrate to Ed25519 to enable userDataStream.subscribe"
                    .into(),
            )
        })?;
        let (tx, rx) = mpsc::channel(256);
        let ws = user_data_ws_api::BinanceUserDataWsApi::new_spot(ed25519.clone());
        ws.start(tx);
        tracing::info!(
            "[BinanceExchange] Spot order WS API started (Ed25519, userDataStream.subscribe)"
        );
        Ok(rx)
    }

    // ---- listenKey 订单 WS（合约用户数据流 / 现货 HMAC 降级路径）----

    async fn start_listenkey_order_ws(
        &self,
        listen_key_hint: Option<&str>,
    ) -> Result<mpsc::Receiver<WsFeedEvent>, ExchangeError> {
        // 1. 获取 listenKey：优先使用 hint，否则按市场类型创建
        let listen_key = match listen_key_hint {
            Some(k) => k.to_string(),
            None => self.create_listen_key().await?,
        };

        // 2. 按市场类型构造 BinanceUserDataWs
        let mut ws = if self.is_perpetual() {
            user_data_ws::BinanceUserDataWs::new_perpetual(listen_key.clone())
        } else {
            user_data_ws::BinanceUserDataWs::new_spot(listen_key.clone())
        };

        // 3. 获取 running flag 引用（keepalive task 据此判断 WS 是否已退出）
        let ws_running = ws.running_handle();

        // 4. 启动 WS 并返回 receiver
        let (tx, rx) = mpsc::channel(256);
        ws.start(tx).await;
        tracing::info!(
            market_type = ?self.market_type,
            "[BinanceExchange] listenKey order WS started"
        );

        // 5. spawn listenKey REST keepalive task
        //    币安要求：现货 30 分钟内、合约 60 分钟内 keepalive 一次，否则 listenKey 失效。
        //    取保守间隔（窗口的 1/2）以容忍网络抖动。
        let client = self.client.clone();
        let signer = Arc::clone(&self.signer);
        let is_perpetual = self.is_perpetual();
        let keepalive_interval = if is_perpetual {
            Duration::from_secs(30 * 60) // 合约：30 分钟（窗口 60 分钟的 1/2）
        } else {
            Duration::from_secs(15 * 60) // 现货：15 分钟（窗口 30 分钟的 1/2）
        };

        tokio::spawn(async move {
            let mut tick = tokio::time::interval(keepalive_interval);
            // 跳过第一次立即触发（WS 刚启动，listenKey 刚创建，无需立即 keepalive）
            tick.tick().await;

            loop {
                if !ws_running.load(Ordering::Relaxed) {
                    tracing::debug!(
                        "[BinanceExchange] listenKey keepalive task exiting: WS stopped"
                    );
                    return;
                }
                tick.tick().await;
                if !ws_running.load(Ordering::Relaxed) {
                    tracing::debug!(
                        "[BinanceExchange] listenKey keepalive task exiting: WS stopped"
                    );
                    return;
                }
                let result = if is_perpetual {
                    fapi::keepalive_listen_key(&client, signer.as_ref(), &listen_key).await
                } else {
                    api::keepalive_listen_key(&client, signer.as_ref(), &listen_key).await
                };
                match result {
                    Ok(()) => {
                        tracing::debug!("[BinanceExchange] listenKey keepalive succeeded");
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
        if self.is_perpetual() {
            fapi::ping(&self.client).await
        } else {
            api::ping(&self.client).await
        }
    }
}

// ============================================================
// Test modules (_tests suffix pattern)
// ============================================================
#[cfg(test)]
mod mod_tests;
#[cfg(test)]
mod orderbook_ws_tests;
#[cfg(test)]
mod user_data_ws_api_tests;
