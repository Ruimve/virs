use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use tokio::sync::{broadcast, mpsc, RwLock};

// Re-export for convenience
use crate::ws_manager::{
    MessageOutcome, WsCommand as ManagerWsCommand, WsHandler, WsManager, WsManagerConfig,
    WsManagerEvent,
};
use crate::ws_types::KlineWsClient;
pub use crate::ws_types::{Candle, WsCandleUpdate, WsEvent};

pub(crate) fn binance_ws_symbol(symbol: &str) -> String {
    symbol.replace('/', "").to_lowercase()
}

/// Binance WS 推送两种格式：
/// 1. 单流格式: {"e":"kline", "s":"BTCUSDT", "k":{...}}
/// 2. 组合流格式: {"stream":"btcusdt@kline_1m", "data":{"e":"kline","k":{...}}}
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BinanceKlineMessage {
    #[allow(dead_code)]
    pub(crate) stream: Option<String>,
    /// 组合流格式: data 字段包含完整的 kline 事件
    pub(crate) data: Option<BinanceKlineData>,
    /// 单流格式: 顶层直接包含 kline 事件字段
    #[serde(rename = "e")]
    pub(crate) event_type_flat: Option<String>,
    /// T8 FAIL fix: 单流格式的事件时间（E 字段），此前缺失导致 event_time 恒为 0
    #[serde(rename = "E")]
    pub(crate) event_time_flat: Option<i64>,
    #[serde(rename = "s")]
    pub(crate) symbol_flat: Option<String>,
    #[serde(rename = "k")]
    pub(crate) kline_flat: Option<BinanceKlineInner>,
}

impl BinanceKlineMessage {
    /// 提取 kline 数据，兼容单流和组合流两种格式
    pub(crate) fn into_kline_data(self) -> Option<BinanceKlineData> {
        if let Some(data) = self.data {
            Some(data)
        } else if let Some(et) = self.event_type_flat.as_deref() {
            if et == "kline" {
                if self.symbol_flat.is_none() {
                    tracing::warn!("Kline WS message missing symbol — skipping kline");
                    return None;
                }
                // T8 FAIL fix: 使用实际解析到的 E 字段，而非硬编码 0
                let event_time = self.event_time_flat.unwrap_or(0);
                self.kline_flat.map(|kline| BinanceKlineData {
                    event_type: et.to_string(),
                    event_time,
                    kline,
                })
            } else {
                None
            }
        } else {
            None
        }
    }
}

/// WS 消息延迟告警阈值（毫秒）
pub(crate) const KLINE_WS_DELAY_THRESHOLD_MS: i64 = 5_000;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BinanceKlineData {
    #[serde(rename = "e")]
    pub(crate) event_type: String,
    #[serde(rename = "E")]
    pub(crate) event_time: i64,
    #[serde(rename = "k")]
    pub(crate) kline: BinanceKlineInner,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BinanceKlineInner {
    #[serde(rename = "t")]
    pub(crate) start_time: i64,
    #[serde(rename = "T")]
    pub(crate) end_time: i64,
    #[serde(rename = "s")]
    pub(crate) symbol: String,
    #[allow(dead_code)]
    #[serde(rename = "i")]
    pub(crate) interval: String,
    #[serde(rename = "o")]
    pub(crate) open: String,
    #[serde(rename = "h")]
    pub(crate) high: String,
    #[serde(rename = "l")]
    pub(crate) low: String,
    #[serde(rename = "c")]
    pub(crate) close: String,
    #[serde(rename = "v")]
    pub(crate) volume: String,
    #[serde(rename = "n")]
    pub(crate) trades: i64,
    #[serde(rename = "x")]
    pub(crate) closed: bool,
    #[serde(rename = "q")]
    pub(crate) quote_volume: String,
}

impl BinanceKlineData {
    pub(crate) fn to_candle(&self) -> Result<Candle, virs_error::ExchangeError> {
        let symbol = &self.kline.symbol;
        let parse = |field: &str, raw: &str| -> Result<f64, virs_error::ExchangeError> {
            raw.parse::<f64>().map_err(|e| {
                tracing::error!(symbol = %symbol, field = field, raw = %raw, error = %e, "Failed to parse kline OHLCV field — returning NoData instead of 0.0");
                virs_error::ExchangeError::no_data(format!(
                    "kline {field} parse failed for {symbol}: {raw} ({e})"
                ))
            })
        };
        Ok(Candle {
            open_time: self.kline.start_time,
            close_time: self.kline.end_time,
            open: parse("open", &self.kline.open)?,
            high: parse("high", &self.kline.high)?,
            low: parse("low", &self.kline.low)?,
            close: parse("close", &self.kline.close)?,
            volume: parse("volume", &self.kline.volume)?,
            quote_volume: parse("quote_volume", &self.kline.quote_volume)?,
            trades: self.kline.trades,
            closed: self.kline.closed,
        })
    }

    pub(crate) fn ws_symbol(&self) -> &str {
        &self.kline.symbol
    }
}

// ============================================================
// KlineWsHandler: WsHandler 实现
// ============================================================

/// Binance Kline WS 的 [`WsHandler`] 实现
///
/// 管理动态订阅状态（`subscriptions` + `symbol_map`），
/// 在 `on_connected` 时自动恢复所有订阅。
///
/// 改进（对比原始实现）：
/// - 连接超时（10s）防止 `connect_async` 挂起
/// - Pong 超时（90s）检测半开连接
/// - 背压容忍：broadcast channel 满时 warn 不停止
/// - 熔断：100 次重试后触发 CircuitBreaker
/// - 统一 `ConnectionChanged` 事件替代 `WsEvent::Reconnected`
/// - 退避 jitter 修正（避免截断为 0）
pub struct KlineWsHandler {
    ws_url: String,
    /// 当前订阅列表 — 重连时通过 on_connected 恢复
    pub(crate) subscriptions: Arc<RwLock<Vec<String>>>,
    /// Binance symbol → 原始 symbol 映射
    pub(crate) symbol_map: Arc<RwLock<HashMap<String, String>>>,
    /// JSON-RPC 请求 ID
    request_id: Arc<AtomicU64>,
}

impl KlineWsHandler {
    pub fn new(ws_url: String) -> Self {
        Self {
            ws_url,
            subscriptions: Arc::new(RwLock::new(Vec::new())),
            symbol_map: Arc::new(RwLock::new(HashMap::new())),
            request_id: Arc::new(AtomicU64::new(1)),
        }
    }
}

#[async_trait]
impl WsHandler<WsEvent> for KlineWsHandler {
    fn base_url(&self) -> &str {
        &self.ws_url
    }

    fn supports_commands(&self) -> bool {
        true
    }

    async fn on_message(&self, text: &str) -> Result<MessageOutcome<WsEvent>, virs_error::ExchangeError> {
        let bmsg: BinanceKlineMessage = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(_) => {
                tracing::warn!(
                    preview = &text[..text.len().min(200)],
                    "[KlineWs] Failed to parse WS message"
                );
                return Ok(MessageOutcome::Continue(vec![]));
            }
        };

        if let Some(data) = bmsg.into_kline_data() {
            if data.event_type == "kline" {
                // 延迟检测
                if data.event_time > 0 {
                    let local_now = chrono::Utc::now().timestamp_millis();
                    let delay_ms = local_now - data.event_time;
                    if delay_ms > KLINE_WS_DELAY_THRESHOLD_MS {
                        tracing::warn!(
                            delay_ms = delay_ms,
                            event_time = data.event_time,
                            local_time = local_now,
                            symbol = %data.kline.symbol,
                            "[KlineWs] Message delay exceeds threshold"
                        );
                    }
                }

                let raw_sym = data.ws_symbol().to_lowercase();
                let original_symbol = {
                    let map = self.symbol_map.read().await;
                    map.get(&raw_sym).cloned().unwrap_or_else(|| raw_sym.clone())
                };

                let candle = match data.to_candle() {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!(
                            symbol = %data.ws_symbol(),
                            error = %e,
                            "Failed to parse kline — skipping this candle update"
                        );
                        return Ok(MessageOutcome::Continue(vec![]));
                    }
                };

                return Ok(MessageOutcome::Continue(vec![WsEvent::Candle(
                    WsCandleUpdate {
                        symbol: original_symbol,
                        candle,
                    },
                )]));
            }
        } else {
            // 订阅确认/错误响应（无 kline 数据）
            if let Ok(resp) = serde_json::from_str::<serde_json::Value>(text) {
                if let Some(code) = resp.get("code") {
                    tracing::error!(
                        id = ?resp.get("id"),
                        code = ?code,
                        msg = ?resp.get("msg"),
                        "[KlineWs] Subscription rejected by Binance"
                    );
                } else if resp.get("result").is_some() {
                    tracing::info!(
                        id = ?resp.get("id"),
                        "[KlineWs] Subscription confirmed by Binance"
                    );
                }
            }
        }

        Ok(MessageOutcome::Continue(vec![]))
    }

    async fn on_connected(&self, _is_reconnect: bool) -> Vec<String> {
        // 重连后自动恢复所有订阅
        let subs_vec = self.subscriptions.read().await.clone();
        if subs_vec.is_empty() {
            return vec![];
        }

        let id = self.request_id.fetch_add(1, Ordering::Relaxed);
        let msg = serde_json::json!({
            "method": "SUBSCRIBE",
            "params": subs_vec,
            "id": id
        });

        tracing::info!(
            id = id,
            count = subs_vec.len(),
            "[KlineWs] Batch subscription request sent on connect"
        );

        vec![msg.to_string()]
    }

    async fn on_disconnected(&self) {
        // 订阅状态保留在 subscriptions 中，重连时 on_connected 恢复
    }

    async fn on_command(&self, cmd: ManagerWsCommand) -> Option<String> {
        let (method, stream_name) = match cmd {
            ManagerWsCommand::Subscribe(s) => ("SUBSCRIBE", s),
            ManagerWsCommand::Unsubscribe(s) => ("UNSUBSCRIBE", s),
        };

        let id = self.request_id.fetch_add(1, Ordering::Relaxed);
        let msg = serde_json::json!({
            "method": method,
            "params": [stream_name.clone()],
            "id": id
        });

        tracing::info!(
            id = id,
            method = method,
            stream = %stream_name,
            "[KlineWs] Dynamic {} request sent",
            method
        );

        Some(msg.to_string())
    }
}

// ============================================================
// KlineWs: 委托给 WsManager 的薄包装
// ============================================================

/// Binance Kline WS 客户端
///
/// 内部委托给 [`WsManager<WsEvent>`]，仅保留对外 API 兼容性。
///
/// 改进（对比原始实现）：
/// - 连接超时（10s）防止 `connect_async` 挂起
/// - Pong 超时（90s）检测半开连接
/// - 背压容忍：broadcast channel 满时 warn 不停止（原始实现直接停止 WS）
/// - 熔断：100 次重试后触发 CircuitBreaker
/// - 统一 `ConnectionChanged` 事件
pub struct KlineWs {
    manager: WsManager<WsEvent>,
    pub(crate) handler: Arc<KlineWsHandler>,
    pub(crate) ws_url: String,
}

impl KlineWs {
    pub fn new(ws_url: String) -> Self {
        let handler = Arc::new(KlineWsHandler::new(ws_url.clone()));
        let config = WsManagerConfig::default();

        Self {
            manager: WsManager::new(config, handler.clone()),
            handler,
            ws_url,
        }
    }

    pub fn new_perpetual(_proxy_url: Option<&str>) -> Self {
        Self::new("wss://fstream.binance.com/market/ws".to_string())
    }

    /// 返回 running flag 引用
    pub fn running_handle(&self) -> Arc<AtomicBool> {
        self.manager.running_handle()
    }
}

#[async_trait]
impl KlineWsClient for KlineWs {
    async fn start(&mut self, update_tx: broadcast::Sender<WsEvent>) {
        // WsManager 发出 WsManagerEvent<WsEvent>，需要桥接到 broadcast<WsEvent>
        let (manager_tx, mut manager_rx) = mpsc::channel::<WsManagerEvent<WsEvent>>(256);

        self.manager.start(manager_tx).await;

        // forwarder task: WsManagerEvent<WsEvent> → broadcast<WsEvent>
        tokio::spawn(async move {
            while let Some(ev) = manager_rx.recv().await {
                let ws_event = match ev {
                    WsManagerEvent::Message(e) => e,
                    WsManagerEvent::ConnectionChanged {
                        connected: true,
                        is_reconnect: true,
                    } => WsEvent::Reconnected,
                    WsManagerEvent::ConnectionChanged {
                        connected: true,
                        is_reconnect: false,
                    } => {
                        // 首次连接不发 Reconnected
                        continue;
                    }
                    WsManagerEvent::ConnectionChanged {
                        connected: false, ..
                    } => {
                        // 断连不发事件（原始实现也没有断连事件）
                        continue;
                    }
                    WsManagerEvent::CircuitBreakerTripped { retry_count } => {
                        tracing::error!(
                            retry_count = retry_count,
                            "[KlineWs] Circuit breaker tripped — WS stopped after max retries"
                        );
                        continue;
                    }
                };
                // broadcast: 满时 warn 不停止（背压容忍 — 原始实现直接停止 WS）
                if update_tx.send(ws_event).is_err() {
                    tracing::warn!("[KlineWs] All receivers dropped, stopping forwarder");
                    break;
                }
            }
        });
    }

    async fn stop(&mut self) {
        self.manager.stop().await;
    }

    async fn subscribe(&self, symbol: &str) {
        let stream_name = format!("{}@kline_1m", binance_ws_symbol(symbol));
        let ws_sym = binance_ws_symbol(symbol);

        {
            let mut map = self.handler.symbol_map.write().await;
            map.insert(ws_sym, symbol.to_string());
        }

        let mut subs = self.handler.subscriptions.write().await;
        if !subs.contains(&stream_name) {
            subs.push(stream_name.clone());
            drop(subs);
            self.manager
                .send_command(ManagerWsCommand::Subscribe(stream_name))
                .await;
        }
    }

    async fn unsubscribe(&self, symbol: &str) {
        let stream_name = format!("{}@kline_1m", binance_ws_symbol(symbol));
        let ws_sym = binance_ws_symbol(symbol);

        {
            let mut map = self.handler.symbol_map.write().await;
            map.remove(&ws_sym);
        }

        let mut subs = self.handler.subscriptions.write().await;
        let existed = subs.iter().any(|s| s == &stream_name);
        subs.retain(|s| s != &stream_name);
        drop(subs);
        if existed {
            self.manager
                .send_command(ManagerWsCommand::Unsubscribe(stream_name))
                .await;
        }
    }

    fn is_running(&self) -> bool {
        self.manager.is_running()
    }
}
