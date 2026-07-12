//! Binance order book WebSocket client.
//!
//! Subscribes to `<symbol>@depth20@500ms` partial book depth streams
//! (top 20 bid/ask levels, pushed every 500ms).
//!
//! 内部委托给 [`WsManager<WsOrderBookEvent>`]，仅保留对外 API 兼容性。
//!
//! Stream formats:
//! - Perpetual partial book depth: { "e":"depthUpdate", "E":..., "T":..., "s":"BTCUSDT",
//!   "U":..., "u":..., "pu":..., "b":[[p,a]], "a":[[p,a]] }
//! - Combined stream wrapper:    { "stream": "<name>", "data": <payload> }

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use tokio::sync::{broadcast, mpsc, RwLock};

use crate::ws_manager::{
    MessageOutcome, WsCommand as ManagerWsCommand, WsHandler, WsManager, WsManagerConfig,
    WsManagerEvent,
};
use crate::ws_types::{OrderBookLevel, OrderBookWsClient, WsOrderBookEvent, WsOrderBookUpdate};

fn binance_ws_symbol(symbol: &str) -> String {
    symbol.replace('/', "").to_lowercase()
}

/// WS 消息延迟告警阈值（毫秒）— 订单簿推送间隔 500ms，阈值设为 2 秒
pub(crate) const ORDERBOOK_WS_DELAY_THRESHOLD_MS: i64 = 2_000;

/// Binance WS 推送两种格式（与 kline 一致）：
/// 1. 单流格式: 顶层直接是 payload
/// 2. 组合流格式: {"stream":"<name>", "data": <payload>}
///
/// Perpetual payload 字段: b / a / e / E / T / s / U / u / pu
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BinanceDepthMessage {
    stream: Option<String>,
    /// 组合流格式: data 字段包含完整 payload
    data: Option<serde_json::Value>,
    /// 单流 perpetual 格式: 顶层 b/a
    #[serde(rename = "e")]
    #[allow(dead_code)]
    event_type_flat: Option<String>,
    #[serde(rename = "E")]
    event_time_flat: Option<i64>,
    #[serde(rename = "T")]
    #[allow(dead_code)]
    transaction_time_flat: Option<i64>,
    #[serde(rename = "s")]
    symbol_flat: Option<String>,
    #[serde(rename = "b")]
    bids_perp_flat: Option<Vec<[String; 2]>>,
    #[serde(rename = "a")]
    asks_perp_flat: Option<Vec<[String; 2]>>,
}

/// T9 WARN fix: 替代 6-tuple 的命名结构体，避免 bids/asks、stream/symbol 位置混淆
pub(crate) struct ParsedDepth {
    pub bids: Vec<[String; 2]>,
    pub asks: Vec<[String; 2]>,
    pub stream_name: Option<String>,
    pub symbol: Option<String>,
    pub timestamp_ms: i64,
    pub last_update_id: Option<i64>,
}

impl BinanceDepthMessage {
    /// 提取订单簿数据，兼容单流/组合流
    pub(crate) fn into_depth(self) -> Option<ParsedDepth> {
        let stream = self.stream.clone();

        // 组合流：解析 data
        if let Some(data) = self.data {
            if let Some(pd) = parse_payload(&data) {
                return Some(ParsedDepth {
                    stream_name: stream,
                    ..pd
                });
            }
            return None;
        }

        // 单流 perpetual: b/a at top level
        if let (Some(bids), Some(asks)) = (self.bids_perp_flat, self.asks_perp_flat) {
            let ts = self.event_time_flat.unwrap_or_else(|| {
                tracing::warn!("orderbook_ws: event_time_flat is None — using 0 as fallback timestamp");
                0
            });
            return Some(ParsedDepth {
                bids,
                asks,
                stream_name: stream,
                symbol: self.symbol_flat,
                timestamp_ms: ts,
                last_update_id: None,
            });
        }

        None
    }
}

/// 解析 payload（组合流的 data 字段或单流的顶层）
pub(crate) fn parse_payload(v: &serde_json::Value) -> Option<ParsedDepth> {
    // Perpetual format: b/a
    if let (Some(bids), Some(asks)) = (v.get("b"), v.get("a")) {
        let bids = parse_levels(bids)?;
        let asks = parse_levels(asks)?;
        let sym = v.get("s").and_then(|s| s.as_str()).map(String::from);
        let ts = v.get("E").and_then(|t| t.as_i64()).unwrap_or_else(|| {
            tracing::warn!("orderbook_ws: event time 'E' missing in perpetual payload — using 0 as fallback");
            0
        });
        return Some(ParsedDepth {
            bids,
            asks,
            stream_name: None,
            symbol: sym,
            timestamp_ms: ts,
            last_update_id: None,
        });
    }
    None
}

pub(crate) fn parse_levels(v: &serde_json::Value) -> Option<Vec<[String; 2]>> {
    let arr = v.as_array()?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let pair = item.as_array()?;
        if pair.len() < 2 {
            return None;
        }
        let p = pair[0].as_str().or_else(|| pair[0].as_f64().map(|_| ""))?;
        let a = pair[1].as_str().or_else(|| pair[1].as_f64().map(|_| ""))?;
        // If numbers, convert to string via serde_json
        let p = if p.is_empty() {
            pair[0].as_f64().map(|n| n.to_string())?
        } else {
            p.to_string()
        };
        let a = if a.is_empty() {
            pair[1].as_f64().map(|n| n.to_string())?
        } else {
            a.to_string()
        };
        out.push([p, a]);
    }
    Some(out)
}

pub(crate) fn to_levels(raw: &[[String; 2]]) -> Vec<OrderBookLevel> {
    raw.iter()
        .filter_map(|[p, a]| {
            let price: f64 = p.parse().ok()?;
            let amount: f64 = a.parse().ok()?;
            if amount > 0.0 {
                Some(OrderBookLevel { price, amount })
            } else {
                None
            }
        })
        .collect()
}

// ============================================================
// OrderBookWsHandler: WsHandler 实现
// ============================================================

/// Binance OrderBook WS 的 [`WsHandler`] 实现
///
/// 改进（对比原始实现）：
/// - 连接超时（10s）防止 `connect_async` 挂起
/// - Pong 超时（90s）检测半开连接
/// - **新增延迟检测**（P2 修复）：原始实现完全缺失，现添加 2s 阈值告警
/// - 背压容忍：broadcast channel 满时 warn 不停止
/// - 熔断：100 次重试后触发 CircuitBreaker
pub struct OrderBookWsHandler {
    ws_url: String,
    /// 当前订阅列表 — 重连时通过 on_connected 恢复
    pub(crate) subscriptions: Arc<RwLock<Vec<String>>>,
    /// Binance symbol → 原始 symbol 映射
    pub(crate) symbol_map: Arc<RwLock<HashMap<String, String>>>,
    /// JSON-RPC 请求 ID
    request_id: Arc<AtomicU64>,
}

impl OrderBookWsHandler {
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
impl WsHandler<WsOrderBookEvent> for OrderBookWsHandler {
    fn base_url(&self) -> &str {
        &self.ws_url
    }

    fn supports_commands(&self) -> bool {
        true
    }

    async fn on_message(
        &self,
        text: &str,
    ) -> Result<MessageOutcome<WsOrderBookEvent>, virs_error::ExchangeError> {
        let bmsg: BinanceDepthMessage = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(_) => {
                tracing::warn!(
                    preview = &text[..text.len().min(200)],
                    "[OrderBookWs] Failed to parse WS message"
                );
                return Ok(MessageOutcome::Continue(vec![]));
            }
        };

        if let Some(pd) = bmsg.into_depth() {
            // P2 修复：延迟检测 — 原始实现完全缺失
            if pd.timestamp_ms > 0 {
                let local_now = chrono::Utc::now().timestamp_millis();
                let delay_ms = local_now - pd.timestamp_ms;
                if delay_ms > ORDERBOOK_WS_DELAY_THRESHOLD_MS {
                    tracing::warn!(
                        delay_ms = delay_ms,
                        event_time = pd.timestamp_ms,
                        local_time = local_now,
                        "[OrderBookWs] Message delay exceeds threshold"
                    );
                }
            }

            // Resolve unified symbol
            let original_symbol =
                resolve_symbol(pd.stream_name.as_deref(), pd.symbol.as_deref(), &self.symbol_map)
                    .await;

            if let Some(symbol) = original_symbol {
                let bids = to_levels(&pd.bids);
                let asks = to_levels(&pd.asks);
                if !bids.is_empty() || !asks.is_empty() {
                    return Ok(MessageOutcome::Continue(vec![
                        WsOrderBookEvent::OrderBook(WsOrderBookUpdate {
                            symbol,
                            bids,
                            asks,
                            timestamp: pd.timestamp_ms,
                            last_update_id: pd.last_update_id,
                        }),
                    ]));
                }
            }
        } else {
            // 订阅确认/错误响应
            if let Ok(resp) = serde_json::from_str::<serde_json::Value>(text) {
                if let Some(code) = resp.get("code") {
                    tracing::error!(
                        id = ?resp.get("id"),
                        code = ?code,
                        msg = ?resp.get("msg"),
                        "[OrderBookWs] Subscription rejected by Binance"
                    );
                } else if resp.get("result").is_some() {
                    tracing::info!(
                        id = ?resp.get("id"),
                        "[OrderBookWs] Subscription confirmed by Binance"
                    );
                }
            }
        }

        Ok(MessageOutcome::Continue(vec![]))
    }

    async fn on_connected(&self, _is_reconnect: bool) -> Vec<String> {
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
            "[OrderBookWs] Batch subscription request sent on connect"
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
            "[OrderBookWs] Dynamic {} request sent",
            method
        );

        Some(msg.to_string())
    }
}

// ============================================================
// OrderBookWs: 委托给 WsManager 的薄包装
// ============================================================

/// Binance OrderBook WS 客户端
///
/// 内部委托给 [`WsManager<WsOrderBookEvent>`]，仅保留对外 API 兼容性。
pub struct OrderBookWs {
    manager: WsManager<WsOrderBookEvent>,
    pub(crate) handler: Arc<OrderBookWsHandler>,
    pub(crate) ws_url: String,
}

impl OrderBookWs {
    pub fn new(ws_url: String) -> Self {
        let handler = Arc::new(OrderBookWsHandler::new(ws_url.clone()));
        let config = WsManagerConfig::default();

        Self {
            manager: WsManager::new(config, handler.clone()),
            handler,
            ws_url,
        }
    }

    pub fn new_perpetual(_proxy_url: Option<&str>) -> Self {
        Self::new("wss://fstream.binance.com/public/stream".to_string())
    }

    pub fn running_handle(&self) -> Arc<AtomicBool> {
        self.manager.running_handle()
    }
}

#[async_trait]
impl OrderBookWsClient for OrderBookWs {
    async fn start(&mut self, update_tx: broadcast::Sender<WsOrderBookEvent>) {
        let (manager_tx, mut manager_rx) = mpsc::channel::<WsManagerEvent<WsOrderBookEvent>>(256);

        self.manager.start(manager_tx).await;

        tokio::spawn(async move {
            while let Some(ev) = manager_rx.recv().await {
                let ws_event = match ev {
                    WsManagerEvent::Message(e) => e,
                    WsManagerEvent::ConnectionChanged {
                        connected: true,
                        is_reconnect: true,
                    } => WsOrderBookEvent::Reconnected,
                    WsManagerEvent::ConnectionChanged {
                        connected: true,
                        is_reconnect: false,
                    } => {
                        continue;
                    }
                    WsManagerEvent::ConnectionChanged {
                        connected: false, ..
                    } => {
                        continue;
                    }
                    WsManagerEvent::CircuitBreakerTripped { retry_count } => {
                        tracing::error!(
                            retry_count = retry_count,
                            "[OrderBookWs] Circuit breaker tripped — WS stopped after max retries"
                        );
                        continue;
                    }
                };
                if update_tx.send(ws_event).is_err() {
                    tracing::warn!("[OrderBookWs] All receivers dropped, stopping forwarder");
                    break;
                }
            }
        });
    }

    async fn stop(&mut self) {
        self.manager.stop().await;
    }

    async fn subscribe(&self, symbol: &str) {
        let stream_name = format!("{}@depth20@500ms", binance_ws_symbol(symbol));
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
        let stream_name = format!("{}@depth20@500ms", binance_ws_symbol(symbol));
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

/// Resolve unified symbol from stream name, payload symbol, or symbol_map.
///
/// Resolution order:
/// 1. Stream name (e.g. "btcusdt@depth20@500ms" → "btcusdt" → lookup in map)
/// 2. Payload symbol (perpetual `s` field, e.g. "BTCUSDT" → lowercase → lookup)
/// 3. Single-subscription fallback (if only one symbol subscribed)
async fn resolve_symbol(
    stream_name: Option<&str>,
    sym_from_payload: Option<&str>,
    symbol_map: &Arc<RwLock<HashMap<String, String>>>,
) -> Option<String> {
    let map = symbol_map.read().await;

    // 1. Try stream name: "btcusdt@depth20@500ms" → "btcusdt"
    if let Some(stream) = stream_name {
        if let Some(symbol_part) = stream.split('@').next() {
            let key = symbol_part.to_lowercase();
            if let Some(unified) = map.get(&key) {
                return Some(unified.clone());
            }
        }
    }

    // 2. Try payload symbol (perpetual `s` field)
    if let Some(s) = sym_from_payload {
        let key = s.to_lowercase();
        if let Some(unified) = map.get(&key) {
            return Some(unified.clone());
        }
    }

    // 3. Single-subscription fallback (no stream name or payload symbol)
    if map.len() == 1 {
        return map.values().next().cloned();
    }

    None
}
