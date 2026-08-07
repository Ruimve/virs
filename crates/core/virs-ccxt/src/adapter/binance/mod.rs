use tracing::{info, warn};

pub mod fapi;
pub mod kline_ws;
pub mod orderbook_ws;
pub mod sapi;
pub mod user_data_ws;
pub mod user_data_ws_events;

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;
use ed25519_dalek::pkcs8::DecodePrivateKey;
use tokio::sync::mpsc;
use virs_task::{spawn_periodic, TaskHandle};

use crate::auth::{hmac_sha256_hex, insert_header, SignedRequest, Signer};
use crate::types::*;
use crate::ExchangeClient;
use virs_error::{ExchangeError, VirsError};
use virs_type::{
    OrderUpdateStream, FundingRate, Kline, Ticker, KlineWsClient, OrderBookWsClient,
    ApiRestrictions, Balance, CcxtOrderStatus, ExchangePe, ExchangePosition, MarginMode,
    MarketType, OrderResult, OrderType, PlaceOrderParams, PositionMode, Side, WsFeedEvent,
};


/* 币安 HMAC-SHA256 签名器，用 api_secret 对请求参数签名，用于 REST API 鉴权 */
pub struct BinanceSigner {
    api_key: String,
    api_secret: String,
    /* 本地与服务器时间偏移（毫秒），签名时校正 timestamp，避免因时钟漂移被拒绝 */
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

/* 请求接收窗口（毫秒），币安允许的最大请求与服务器时间差 */
const RECV_WINDOW: &str = "5000";

/* Ed25519 签名的 base64 签名中包含 +/= 字符，URL 编码后才能安全传输 */
fn url_encode_signature(s: &str) -> String {
    s.replace('+', "%2B")
        .replace('/', "%2F")
        .replace('=', "%3D")
}

/* 时间同步周期：每小时重新校准一次服务器时间偏移 */
const TIME_SYNC_INTERVAL_SECS: u64 = 3600;
/* 时间偏移告警阈值：超过 2 秒说明本地时钟漂移严重，可能导致签名失败 */
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
        /* 加入时间偏移校正后的 timestamp 和 recvWindow，防止因时钟漂移导致签名失效 */
        let timestamp =
            chrono::Utc::now().timestamp_millis() + self.time_offset_ms.load(Ordering::Acquire);
        query_params.push(("recvWindow".into(), RECV_WINDOW.into()));
        query_params.push(("timestamp".into(), timestamp.to_string()));

        /* 将所有参数拼接成 query string 作为签名原文 */
        let query_string = query_params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");

        /* 用 api_secret 对 query string 做 HMAC-SHA256 签名，附加到参数末尾 */
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
        let timestamp =
            chrono::Utc::now().timestamp_millis() + self.time_offset_ms.load(Ordering::Acquire);
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

            query_params = pairs.clone();
            let form_body = pairs
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");
            Some(serde_json::Value::String(form_body))
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

/* 币安 Ed25519 签名器，比 HMAC 性能更高且更安全，优先使用 */
pub struct BinanceEd25519Signer {
    api_key: String,
    signing_key: ed25519_dalek::SigningKey,
    time_offset_ms: Arc<AtomicI64>,
}

impl Clone for BinanceEd25519Signer {
    fn clone(&self) -> Self {
        Self {
            api_key: self.api_key.clone(),
            signing_key: self.signing_key.clone(),
            time_offset_ms: Arc::clone(&self.time_offset_ms),
        }
    }
}

impl BinanceEd25519Signer {
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
        let timestamp =
            chrono::Utc::now().timestamp_millis() + self.time_offset_ms.load(Ordering::Acquire);
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
        let timestamp =
            chrono::Utc::now().timestamp_millis() + self.time_offset_ms.load(Ordering::Acquire);
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
            pairs.push(("signature".into(), signature.clone()));

            query_params = pairs.clone();
            let form_body = pairs
                .iter()
                .map(|(k, v)| {
                    if k == "signature" {
                        format!("{}={}", k, url_encode_signature(v))
                    } else {
                        format!("{}={}", k, v)
                    }
                })
                .collect::<Vec<_>>()
                .join("&");
            Some(serde_json::Value::String(form_body))
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

/* 尝试构建 Ed25519 签名器：若 api_secret 是 PEM 格式或 32 字节 base64 则用 Ed25519，否则回退到 HMAC */
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


/* 币安交易所实现，封装 HTTP 客户端、签名器、市场信息缓存及后台任务（时间同步、listenKey 续期） */
pub struct BinanceExchange {
    client: ExchangeClient,
    signer: Arc<dyn Signer>,
    market_type: MarketType,
    /* 市场信息缓存，避免频繁请求 exchangeInfo 接口 */
    markets_cache: tokio::sync::RwLock<Option<Vec<MarketInfo>>>,
    time_sync_started: AtomicBool,
    listenkey_keepalive_futures_secs: u64,
    time_sync_task: std::sync::Mutex<Option<TaskHandle>>,
    listenkey_task: std::sync::Mutex<Option<TaskHandle>>,
    user_data_ws: std::sync::Mutex<Option<user_data_ws::UserDataWs>>,
}

impl BinanceExchange {
    pub fn new(
        api_key: &str,
        api_secret: &str,
        proxy_url: Option<&str>,
        http_timeout: std::time::Duration,
        connect_timeout: std::time::Duration,
        pool_max_idle_per_host: usize,
        listenkey_keepalive_futures_secs: u64,
    ) -> Result<Self, ExchangeError> {
        let max_concurrent: u32 = 40;
        let client = ExchangeClient::with_api_key(
            max_concurrent,
            proxy_url,
            Some(api_key),
            http_timeout,
            connect_timeout,
            pool_max_idle_per_host,
        )?;

        /* 优先尝试 Ed25519 签名（更安全高效），失败则回退到 HMAC-SHA256 */
        let signer = match try_build_ed25519(api_key, api_secret) {
            Ok(ed) => {
                let arc: Arc<dyn Signer> = Arc::new(ed);
                arc
            }
            Err(_) => Arc::new(BinanceSigner::new(
                api_key.to_string(),
                api_secret.to_string(),
            )) as Arc<dyn Signer>,
        };

        Ok(Self {
            client,
            signer,
            market_type: MarketType::Perpetual,
            markets_cache: tokio::sync::RwLock::new(None),
            time_sync_started: AtomicBool::new(false),
            listenkey_keepalive_futures_secs,
            time_sync_task: std::sync::Mutex::new(None),
            listenkey_task: std::sync::Mutex::new(None),
            user_data_ws: std::sync::Mutex::new(None),
        })
    }

    /* 将币安订单状态字符串映射为统一枚举，未知状态保留原始值便于排查 */
    pub fn parse_order_status(status: &str) -> CcxtOrderStatus {
        match status {
            "NEW" => CcxtOrderStatus::New,
            "PARTIALLY_FILLED" => CcxtOrderStatus::PartiallyFilled,
            "FILLED" => CcxtOrderStatus::Filled,
            "CANCELED" | "CANCELLED" => CcxtOrderStatus::Canceled,
            "EXPIRED" => CcxtOrderStatus::Expired,
            "EXPIRED_IN_MATCH" => CcxtOrderStatus::ExpiredInMatch,
            other => CcxtOrderStatus::Unknown(other.to_string()),
        }
    }

    pub fn parse_order_type(order_type: &str) -> OrderType {
        match order_type {
            "MARKET" => OrderType::Market,
            "LIMIT" => OrderType::Limit,
            "STOP" => OrderType::Stop,
            "STOP_MARKET" => OrderType::StopMarket,
            "TAKE_PROFIT" => OrderType::TakeProfit,
            "TAKE_PROFIT_MARKET" => OrderType::TakeProfitMarket,
            "TRAILING_STOP_MARKET" => OrderType::TrailingStopMarket,
            "LIQUIDATION" => OrderType::Liquidation,
            other => OrderType::Unknown(other.to_string()),
        }
    }

    pub fn side_str(side: &Side) -> String {
        match side {
            Side::Buy => "BUY".to_string(),
            Side::Sell => "SELL".to_string(),
            Side::Unknown(raw) => raw.clone(),
        }
    }

    pub fn order_type_str(order_type: &OrderType) -> String {
        match order_type {
            OrderType::Market => "MARKET".to_string(),
            OrderType::Limit => "LIMIT".to_string(),
            OrderType::Stop => "STOP".to_string(),
            OrderType::StopMarket => "STOP_MARKET".to_string(),
            OrderType::TakeProfit => "TAKE_PROFIT".to_string(),
            OrderType::TakeProfitMarket => "TAKE_PROFIT_MARKET".to_string(),
            OrderType::TrailingStopMarket => "TRAILING_STOP_MARKET".to_string(),
            OrderType::Liquidation => "LIQUIDATION".to_string(),
            OrderType::Unknown(raw) => raw.clone(),
        }
    }


    /* 同步服务器时间：计算本地与交易所时间偏移，并启动每小时定时校准的后台任务 */
    pub async fn sync_time(&self) -> Result<(), ExchangeError> {
        let server_time = fapi::fetch_server_time(&self.client).await?;
        let local_time = chrono::Utc::now().timestamp_millis();
        let offset = server_time - local_time;
        self.signer.set_time_offset(offset);

        if offset.abs() > TIME_OFFSET_WARN_THRESHOLD_MS {
            warn!(
                time_offset_ms = offset,
                threshold_ms = TIME_OFFSET_WARN_THRESHOLD_MS,
                "Server time offset exceeds threshold — clock drift detected"
            );
        }

        info!(time_offset_ms = offset, "Server time synced");

        /* 用 CAS 确保周期性时间同步任务只启动一次 */
        if !self.time_sync_started.swap(true, Ordering::SeqCst) {
            let client = self.client.clone();
            let signer = self.signer.clone();

            let handle = spawn_periodic(
                "time_sync",
                Duration::from_secs(TIME_SYNC_INTERVAL_SECS),
                false,
                move || {
                    let client = client.clone();
                    let signer = signer.clone();
                    async move {
                        let result = fapi::fetch_server_time(&client).await;
                        match result {
                            Ok(server_time) => {
                                let local_time = chrono::Utc::now().timestamp_millis();
                                let offset = server_time - local_time;
                                signer.set_time_offset(offset);

                                if offset.abs() > TIME_OFFSET_WARN_THRESHOLD_MS {
                                    warn!(
                                        time_offset_ms = offset,
                                        threshold_ms = TIME_OFFSET_WARN_THRESHOLD_MS,
                                        "Server time offset exceeds threshold — clock drift detected"
                                    );
                                } else {
                                    info!(
                                        time_offset_ms = offset,
                                        "Server time re-synced successfully"
                                    );
                                }
                            }
                            Err(e) => {
                                warn!(
                                    error = %e,
                                    "Failed to re-sync server time — will retry next cycle"
                                );
                            }
                        }
                    }
                },
            );

            *self.time_sync_task.lock().unwrap() = Some(handle);
        }

        Ok(())
    }


    /* 启动 listenKey 用户数据 WebSocket 并定期续期 listenKey，防止 WS 因 key 过期而断开 */
    async fn start_listenkey_order_ws(
        &self,
        listen_key_hint: Option<&str>,
    ) -> Result<mpsc::Receiver<WsFeedEvent>, ExchangeError> {
        let listen_key = match listen_key_hint {
            Some(k) => k.to_string(),
            None => fapi::create_listen_key(&self.client, self.signer.as_ref()).await?,
        };

        let ws = user_data_ws::UserDataWs::new_perpetual(
            listen_key,
            self.client.clone(),
            Arc::clone(&self.signer),
        );

        let listen_key_handle = ws.listen_key_handle();

        let (tx, rx) = mpsc::channel(256);
        ws.start(tx).await;
        info!("listenKey order WS started (perpetual)");

        let client = self.client.clone();
        let signer = Arc::clone(&self.signer);
        let keepalive_interval = Duration::from_secs(self.listenkey_keepalive_futures_secs);

        /* 定时续期 listenKey：币安 U 本位合约 listenKey 有效期 60 分钟，需在过期前 PUT 续期 */
        let handle = spawn_periodic(
            "listenkey_keepalive",
            keepalive_interval,
            false,
            move || {
                let client = client.clone();
                let signer = Arc::clone(&signer);
                let listen_key_handle = Arc::clone(&listen_key_handle);
                async move {
                    let current_key = listen_key_handle
                        .read()
                        .expect("listenKey RwLock poisoned")
                        .clone();
                    let result =
                        fapi::keepalive_listen_key(&client, signer.as_ref(), &current_key).await;
                    match result {
                        Ok(()) => {}
                        Err(e) => {
                            warn!(
                                error = %e,
                                "listenKey keepalive failed, \
                                 WS may disconnect when listenKey expires"
                            );
                        }
                    }
                }
            },
        );

        *self.listenkey_task.lock().unwrap() = Some(handle);
        *self.user_data_ws.lock().unwrap() = Some(ws);

        Ok(rx)
    }


    /* 获取市场信息（带缓存）：首次从 exchangeInfo 接口拉取后缓存，后续直接返回缓存数据 */
    async fn get_markets_cached(&self) -> Result<Vec<MarketInfo>, ExchangeError> {
        {
            let cache = self.markets_cache.read().await;
            if let Some(ref cached) = *cache {
                return Ok(cached.clone());
            }
        }
        let markets = fapi::fetch_markets(&self.client).await?;
        let mut cache = self.markets_cache.write().await;
        *cache = Some(markets.clone());
        Ok(markets)
    }
}


#[async_trait]
impl ExchangePe for BinanceExchange {
    fn name(&self) -> &str {
        "binance"
    }

    fn market_type(&self) -> MarketType {
        self.market_type
    }

    async fn get_ticker(&self, symbol: &str) -> Result<Ticker, VirsError> {
        fapi::fetch_ticker(&self.client, symbol)
            .await
            .map_err(VirsError::from)
    }

    async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        limit: u32,
        since: Option<i64>,
    ) -> Result<Vec<Kline>, VirsError> {
        fapi::fetch_ohlcv(&self.client, symbol, interval, limit, since)
            .await
            .map_err(VirsError::from)
    }

    async fn get_klines_range(
        &self,
        symbol: &str,
        interval: &str,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<Vec<Kline>, VirsError> {
        /* 分页拉取 K 线数据：每次取 1000 条，以最后一条收盘时间 +1ms 为下一批起点，最多取 10000 条 */
        let mut all = Vec::new();
        let mut current_since = Some(start_ms);
        loop {
            let batch =
                fapi::fetch_ohlcv(&self.client, symbol, interval, 1000, current_since)
                    .await?;
            if batch.is_empty() {
                break;
            }
            let last_close_time = batch.last().unwrap().close_time;
            all.extend(batch);
            if last_close_time >= end_ms {
                break;
            }
            current_since = Some(last_close_time + 1);
            if all.len() >= 10000 {
                break;
            }
        }
        Ok(all)
    }

    async fn get_balance(&self) -> Result<Balance, VirsError> {
        let balances = fapi::fetch_balance(&self.client, self.signer.as_ref()).await?;
        balances
            .into_iter()
            .find(|b| b.asset.eq_ignore_ascii_case("USDT"))
            .ok_or_else(|| {
                VirsError::Exchange(ExchangeError::no_data(
                    "No USDT balance found on Binance Futures".into(),
                ))
            })
    }

    async fn get_positions(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<ExchangePosition>, VirsError> {
        fapi::fetch_positions(&self.client, self.signer.as_ref(), symbol)
            .await
            .map_err(VirsError::from)
    }

    async fn get_funding_rate(&self, symbol: &str) -> Result<FundingRate, VirsError> {
        fapi::fetch_funding_rate(&self.client, symbol)
            .await
            .map_err(VirsError::from)
    }

    async fn get_symbols(&self) -> Result<Vec<String>, VirsError> {
        let markets = self.get_markets_cached().await?;
        Ok(markets.iter().map(|m| m.id.clone()).collect())
    }

    async fn get_min_qty(&self, symbol: &str) -> Result<f64, VirsError> {
        let markets = self.get_markets_cached().await?;
        markets
            .iter()
            .find(|m| m.id.eq_ignore_ascii_case(symbol))
            .and_then(|m| m.min_amount)
            .ok_or_else(|| {
                VirsError::Exchange(ExchangeError::no_data(format!(
                    "No min_qty found for {} on Binance Futures",
                    symbol
                )))
            })
    }

    async fn place_order(&self, params: PlaceOrderParams) -> Result<OrderResult, VirsError> {
        fapi::create_order(&self.client, self.signer.as_ref(), params)
            .await
            .map_err(VirsError::from)
    }

    async fn cancel_order(
        &self,
        symbol: &str,
        order_id: &str,
    ) -> Result<OrderResult, VirsError> {
        fapi::cancel_order(&self.client, self.signer.as_ref(), symbol, order_id)
            .await
            .map_err(VirsError::from)
    }

    async fn cancel_all_orders(&self, symbol: Option<&str>) -> Result<Vec<OrderResult>, VirsError> {
        let sym = symbol.ok_or_else(|| {
            VirsError::Exchange(ExchangeError::InvalidRequest(
                "symbol is required for cancel_all_orders on Binance".into(),
            ))
        })?;
        fapi::cancel_all_orders(&self.client, self.signer.as_ref(), sym)
            .await
            .map_err(VirsError::from)?;
        Ok(Vec::new())
    }

    async fn set_leverage(&self, symbol: &str, leverage: u32) -> Result<(), VirsError> {
        /* 先设置保证金模式为全仓，再设置杠杆倍数（币安要求两者配合使用） */
        fapi::set_margin_type(&self.client, self.signer.as_ref(), symbol, MarginMode::Cross)
            .await
            .map_err(VirsError::from)?;
        fapi::set_leverage(&self.client, self.signer.as_ref(), symbol, leverage)
            .await
            .map_err(VirsError::from)
    }

    async fn get_position_mode(&self) -> Result<PositionMode, VirsError> {
        fapi::get_position_mode(&self.client, self.signer.as_ref())
            .await
            .map_err(VirsError::from)
    }

    async fn create_listen_key(&self) -> Result<String, VirsError> {
        fapi::create_listen_key(&self.client, self.signer.as_ref())
            .await
            .map_err(VirsError::from)
    }

    async fn ping(&self) -> Result<bool, VirsError> {
        fapi::ping(&self.client).await.map_err(VirsError::from)
    }

    async fn get_api_restrictions(&self) -> Result<ApiRestrictions, VirsError> {
        sapi::fetch_api_restrictions(&self.client, self.signer.as_ref())
            .await
            .map_err(VirsError::from)
    }

    async fn subscribe_order_updates(
        &self,
        _symbols: &[&str],
    ) -> Result<OrderUpdateStream, VirsError> {
        let rx = self
            .start_listenkey_order_ws(None)
            .await
            .map_err(VirsError::from)?;
        Ok(Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx)))
    }

    fn create_kline_ws(
        &self,
        proxy: Option<&str>,
    ) -> Result<Arc<tokio::sync::Mutex<dyn KlineWsClient>>, VirsError> {
        Ok(Arc::new(tokio::sync::Mutex::new(
            kline_ws::KlineWs::new_perpetual(proxy),
        )))
    }

    fn create_orderbook_ws(
        &self,
        proxy: Option<&str>,
    ) -> Result<Arc<tokio::sync::Mutex<dyn OrderBookWsClient>>, VirsError> {
        Ok(Arc::new(tokio::sync::Mutex::new(
            orderbook_ws::OrderBookWs::new_perpetual(proxy),
        )))
    }
}

/* 析构时取消后台任务，避免时间同步和 listenKey 续期任务泄漏 */
impl Drop for BinanceExchange {
    fn drop(&mut self) {
        if let Some(h) = self.time_sync_task.lock().unwrap().take() {
            h.cancel();
        }
        if let Some(h) = self.listenkey_task.lock().unwrap().take() {
            h.cancel();
        }
    }
}

#[cfg(test)]
mod kline_ws_tests;
#[cfg(test)]
mod mod_tests;
#[cfg(test)]
mod orderbook_ws_tests;
#[cfg(test)]
mod user_data_ws_tests;
