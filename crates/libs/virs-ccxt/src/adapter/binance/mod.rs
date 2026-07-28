use tracing::info;

pub mod fapi;
pub mod kline_ws;
pub mod orderbook_ws;
pub mod sapi;
pub mod user_data_ws;
pub mod user_data_ws_events;

use async_trait::async_trait;
use base64::Engine;
use ed25519_dalek::pkcs8::DecodePrivateKey;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::auth::{hmac_sha256_hex, insert_header, SignedRequest, Signer};
use crate::types::*;
use crate::{Exchange, ExchangeClient};
use virs_error::ExchangeError;
use virs_types::WsFeedEvent;

// 币安 HMAC-SHA256 签名器
pub struct BinanceSigner {
    api_key: String,
    api_secret: String,

    // 服务器时间偏移(毫秒)，用于签名时校正时间戳
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

// 接收窗口5秒
const RECV_WINDOW: &str = "5000";

// Ed25519签名URL编码: Base64中的+→%2B, /→%2F, =→%3D
fn url_encode_signature(s: &str) -> String {
    s.replace('+', "%2B")
        .replace('/', "%2F")
        .replace('=', "%3D")
}

// 时间同步间隔1小时
const TIME_SYNC_INTERVAL_SECS: u64 = 3600;

// 时间偏移告警阈值2秒
const TIME_OFFSET_WARN_THRESHOLD_MS: i64 = 2_000;

impl Signer for BinanceSigner {
    fn set_time_offset(&self, offset_ms: i64) {
        self.time_offset_ms.store(offset_ms, Ordering::Release);
    }

    fn get_time_offset(&self) -> i64 {
        self.time_offset_ms.load(Ordering::Acquire)
    }

    // 签名GET请求：追加recvWindow和timestamp，HMAC-SHA256签名后追加signature，设置x-mbx-apikey header
    fn sign_get(
        &self,
        _path: &str,
        query_params: &mut Vec<(String, String)>,
    ) -> Result<SignedRequest, ExchangeError> {
        // 计算校正后的时间戳
        let timestamp =
            chrono::Utc::now().timestamp_millis() + self.time_offset_ms.load(Ordering::Acquire);
        query_params.push(("recvWindow".into(), RECV_WINDOW.into()));
        query_params.push(("timestamp".into(), timestamp.to_string()));

        // 拼接query string用于签名
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

    // 签名POST请求：将JSON body展平为kv对，追加recvWindow和timestamp，拼接为query string签名，整体作为application/x-www-form-urlencoded body发送
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

        // 将JSON对象展平为kv对并签名
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

            // 拼接为query string并签名
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

// 币安 Ed25519 签名器(比HMAC更安全)
pub struct BinanceEd25519Signer {
    api_key: String,
    signing_key: ed25519_dalek::SigningKey,

    // 服务器时间偏移(毫秒)
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
    // 从PEM格式私钥创建
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

    // 从Base64种子创建(32字节)
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

    // Ed25519签名，输出Base64字符串
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

    // Ed25519签名GET请求：追加recvWindow/timestamp，签名后追加signature
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

    // Ed25519签名POST请求：展平JSON body为kv对，签名后作为form-urlencoded body
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

// 自动检测api_secret格式：PEM→Ed25519，Base64解码为32字节→Ed25519，其他→回退到HMAC
pub(crate) fn try_build_ed25519(
    api_key: &str,
    api_secret: &str,
) -> Result<BinanceEd25519Signer, ExchangeError> {
    let trimmed = api_secret.trim();
    if trimmed.starts_with("-----BEGIN") {
        // PEM格式 → Ed25519
        BinanceEd25519Signer::from_pem(api_key, trimmed)
    } else if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(trimmed) {
        if decoded.len() == 32 {
            // Base64解码为32字节 → Ed25519
            BinanceEd25519Signer::from_seed_b64(api_key, trimmed)
        } else {
            // 非32字节，回退到HMAC
            Err(ExchangeError::Internal(format!(
                "api_secret base64 decodes to {} bytes (not 32), treating as HMAC secret",
                decoded.len()
            )))
        }
    } else {
        // 非Base64，回退到HMAC
        Err(ExchangeError::Internal(
            "api_secret is not base64, treating as HMAC secret".into(),
        ))
    }
}

// 币安交易所适配器
pub struct BinanceExchange {
    client: ExchangeClient,
    signer: Arc<dyn Signer>,

    // 标记周期性时间同步任务是否已启动
    time_sync_started: AtomicBool,

    // 控制周期性时间同步任务的运行状态
    time_sync_running: Arc<AtomicBool>,

    listenkey_keepalive_futures_secs: u64,
}

impl BinanceExchange {
    // 创建实例，优先尝试Ed25519签名器，失败回退到HMAC
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

        // 优先尝试Ed25519，失败则回退到HMAC
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
            time_sync_started: AtomicBool::new(false),
            time_sync_running: Arc::new(AtomicBool::new(false)),
            listenkey_keepalive_futures_secs,
        })
    }

    // 统一符号转币安原生符号: BTC/USDT → BTCUSDT
    pub fn to_native_symbol(symbol: &str) -> String {
        symbol.replace(['/', '-'], "")
    }

    // 币安原生符号转统一符号: BTCUSDT → BTC/USDT
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

    // 币安状态码映射: 与官方文档对齐，未知状态保留原始字符串
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

    // 币安订单类型映射: MARKET/LIMIT/STOP_MARKET等，未知类型保留原始字符串
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

    // 枚举Side转币安字符串: Buy→BUY, Sell→SELL, Unknown→原始值
    pub fn side_str(side: &Side) -> String {
        match side {
            Side::Buy => "BUY".to_string(),
            Side::Sell => "SELL".to_string(),
            Side::Unknown(raw) => raw.clone(),
        }
    }

    // 枚举OrderType转币安字符串，Unknown→原始值
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
}

// 解析订单簿bids/asks数组，每项[price, qty]
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

// Exchange trait 实现: 委托给 fapi/sapi 模块处理具体请求
#[async_trait]
impl Exchange for BinanceExchange {
    fn id(&self) -> &str {
        "binance"
    }
    fn name(&self) -> &str {
        "Binance"
    }

    // 获取24小时行情: GET /fapi/v1/ticker/24hr
    async fn fetch_ticker(&self, symbol: &str) -> Result<CcxtTicker, ExchangeError> {
        fapi::fetch_ticker(&self.client, symbol).await
    }

    // 获取K线数据: GET /fapi/v1/klines
    async fn fetch_ohlcv(
        &self,
        symbol: &str,
        timeframe: &str,
        limit: u32,
        since: Option<i64>,
    ) -> Result<Vec<CcxtKline>, ExchangeError> {
        fapi::fetch_ohlcv(&self.client, symbol, timeframe, limit, since).await
    }

    // 获取订单簿: GET /fapi/v1/depth
    async fn fetch_order_book(
        &self,
        symbol: &str,
        limit: u32,
    ) -> Result<CcxtOrderBook, ExchangeError> {
        fapi::fetch_order_book(&self.client, symbol, limit).await
    }

    // 获取账户余额: GET /fapi/v3/balance (签名)
    async fn fetch_balance(&self) -> Result<Vec<Balance>, ExchangeError> {
        fapi::fetch_balance(&self.client, self.signer.as_ref()).await
    }

    async fn fetch_markets(&self) -> Result<Vec<MarketInfo>, ExchangeError> {
        fapi::fetch_markets(&self.client).await
    }

    // 创建订单: POST /fapi/v1/order (签名)，只返回 orderId + clientOrderId
    async fn create_order(&self, params: PlaceOrderParams) -> Result<OrderResult, ExchangeError> {
        fapi::create_order(&self.client, self.signer.as_ref(), params).await
    }

    // 撤销订单: DELETE /fapi/v1/order (签名)，只返回 orderId + clientOrderId
    async fn cancel_order(
        &self,
        symbol: &str,
        order_id: &str,
    ) -> Result<OrderResult, ExchangeError> {
        fapi::cancel_order(&self.client, self.signer.as_ref(), symbol, order_id).await
    }

    // 批量撤单: DELETE /fapi/v1/allOpenOrders (签名)
    async fn cancel_all_orders(&self, symbol: &str) -> Result<(), ExchangeError> {
        fapi::cancel_all_orders(&self.client, self.signer.as_ref(), symbol).await
    }

    // 设置杠杆: 先POST /fapi/v1/marginType 再POST /fapi/v1/leverage (签名)
    async fn set_leverage(
        &self,
        symbol: &str,
        leverage: u32,
        margin_mode: MarginMode,
    ) -> Result<(), ExchangeError> {
        fapi::set_margin_type(&self.client, self.signer.as_ref(), symbol, margin_mode).await?;
        fapi::set_leverage(&self.client, self.signer.as_ref(), symbol, leverage).await
    }

    // 查询持仓: GET /fapi/v2/positionRisk (签名)
    async fn fetch_positions(&self, symbol: Option<&str>) -> Result<Vec<ExchangePosition>, ExchangeError> {
        fapi::fetch_positions(&self.client, self.signer.as_ref(), symbol).await
    }

    // 查询持仓模式: GET /fapi/v1/positionSide/dual (签名)
    async fn get_position_mode(&self) -> Result<PositionMode, ExchangeError> {
        fapi::get_position_mode(&self.client, self.signer.as_ref()).await
    }

    // 查询资金费率: GET /fapi/v1/premiumIndex
    async fn fetch_funding_rate(&self, symbol: &str) -> Result<CcxtFundingRate, ExchangeError> {
        fapi::fetch_funding_rate(&self.client, symbol).await
    }

    // 创建listenKey: POST /fapi/v1/listenKey (签名)
    async fn create_listen_key(&self) -> Result<String, ExchangeError> {
        fapi::create_listen_key(&self.client, self.signer.as_ref()).await
    }

    async fn fetch_api_restrictions(&self) -> Result<ApiRestrictions, ExchangeError> {
        sapi::fetch_api_restrictions(&self.client, self.signer.as_ref()).await
    }

    // 启动用户数据WS + listenKey保活定时任务
    async fn start_listenkey_order_ws(
        &self,
        listen_key_hint: Option<&str>,
    ) -> Result<mpsc::Receiver<WsFeedEvent>, ExchangeError> {
        // 获取或创建listenKey
        let listen_key = match listen_key_hint {
            Some(k) => k.to_string(),
            None => self.create_listen_key().await?,
        };

        // 创建用户数据WebSocket
        let ws = user_data_ws::UserDataWs::new_perpetual(
            listen_key,
            self.client.clone(),
            Arc::clone(&self.signer),
        );

        // 获取WS运行状态和listenKey句柄
        let ws_running = ws.running_handle();
        let listen_key_handle = ws.listen_key_handle();

        // 启动WS，创建事件通道
        let (tx, rx) = mpsc::channel(256);
        ws.start(tx).await;
        info!("listenKey order WS started (perpetual)");

        // 启动listenKey保活定时任务
        let client = self.client.clone();
        let signer = Arc::clone(&self.signer);
        let keepalive_interval = Duration::from_secs(self.listenkey_keepalive_futures_secs);

        tokio::spawn(async move {
            let mut tick = tokio::time::interval(keepalive_interval);

            tick.tick().await; // 跳过首次立即触发

            loop {
                // 检查WS是否仍在运行
                if !ws_running.load(Ordering::Relaxed) {
                    return;
                }
                tick.tick().await;
                if !ws_running.load(Ordering::Relaxed) {
                    return;
                }

                // 定期续期listenKey
                let current_key = listen_key_handle
                    .read()
                    .expect("listenKey RwLock poisoned")
                    .clone();
                let result =
                    fapi::keepalive_listen_key(&client, signer.as_ref(), &current_key).await;
                match result {
                    Ok(()) => {}
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

    async fn ping(&self) -> Result<bool, ExchangeError> {
        fapi::ping(&self.client).await
    }

    // 时间同步：计算服务器时间偏移，偏移>2000ms告警
    async fn sync_time(&self) -> Result<(), ExchangeError> {
        let server_time = fapi::fetch_server_time(&self.client).await?;
        let local_time = chrono::Utc::now().timestamp_millis();
        let offset = server_time - local_time;
        self.signer.set_time_offset(offset);

        // 偏移超过阈值时告警
        if offset.abs() > TIME_OFFSET_WARN_THRESHOLD_MS {
            tracing::warn!(
                time_offset_ms = offset,
                threshold_ms = TIME_OFFSET_WARN_THRESHOLD_MS,
                "Server time offset exceeds threshold — clock drift detected"
            );
        }

        info!(time_offset_ms = offset, "Server time synced");

        // 首次同步时启动周期性时间同步任务
        if !self.time_sync_started.swap(true, Ordering::SeqCst) {
            self.time_sync_running.store(true, Ordering::Release);
            self.spawn_periodic_time_sync();
        }

        Ok(())
    }
}

impl BinanceExchange {
    // 周期性时间同步(每小时)，防止时钟漂移
    fn spawn_periodic_time_sync(&self) {
        let client = self.client.clone();
        let signer = self.signer.clone();
        let running = self.time_sync_running.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(TIME_SYNC_INTERVAL_SECS));

            interval.tick().await; // 跳过首次立即触发

            loop {
                // 检查运行状态
                if !running.load(Ordering::Acquire) {
                    tracing::info!("[PeriodicSync] Exchange dropped, stopping time sync task");
                    break;
                }
                interval.tick().await;

                if !running.load(Ordering::Acquire) {
                    tracing::info!("[PeriodicSync] Exchange dropped during sleep, stopping");
                    break;
                }
                // 重新获取服务器时间并校正偏移
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

// Drop时停止周期性时间同步任务
impl Drop for BinanceExchange {
    fn drop(&mut self) {
        // 通知后台任务停止
        self.time_sync_running.store(false, Ordering::Release);
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
