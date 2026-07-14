use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use serde::Deserialize;
use tokio::sync::mpsc;
use virs_error::ExchangeError;

// Re-export for convenience
pub use virs_types::WsFeedEvent;
use virs_types::{OrderStatus, PositionSide};

use crate::adapter::binance::fapi;
use crate::adapter::binance::user_data_ws_events::dispatch_event;
use crate::auth::Signer;
use crate::ws_manager::{MessageOutcome, WsHandler, WsManager, WsManagerConfig, WsManagerEvent};
use crate::ExchangeClient;

// ============================================================
// Binance User Data Stream 消息格式
// ============================================================

/// Binance WS 推送两种格式（与 kline 一致）：
/// 1. 单流格式: {"e":"ORDER_TRADE_UPDATE", ...}
/// 2. 组合流格式: {"stream":"<listenKey>", "data":{...}}
#[derive(Debug, Clone, Deserialize)]
pub struct BinanceOrderMessage {
    #[allow(dead_code)]
    pub(crate) stream: Option<String>,
    /// 组合流格式
    pub(crate) data: Option<BinanceOrderData>,
    /// 单流格式
    #[serde(rename = "e")]
    pub(crate) event_type_flat: Option<String>,
    /// 单流格式的完整字段
    #[serde(rename = "E")]
    event_time_flat: Option<i64>,
    #[serde(rename = "o")]
    order_flat: Option<BinanceOrderInner>,
}

impl BinanceOrderMessage {
    /// 返回事件类型（用于判断 ORDER_TRADE_UPDATE / ACCOUNT_UPDATE）
    pub fn event_type(&self) -> Option<&str> {
        self.event_type_flat
            .as_deref()
            .or_else(|| self.data.as_ref().map(|d| d.event_type.as_str()))
    }

    /// 返回事件时间（币安服务器发送时刻，毫秒）
    ///
    /// 用于检测 WS 消息延迟（local_receive_time - event_time）。
    /// 兼容单流格式（event_time_flat）和组合流格式（data.event_time）。
    pub fn event_time(&self) -> Option<i64> {
        self.event_time_flat
            .or_else(|| self.data.as_ref().map(|d| d.event_time))
    }

    /// 转换为 WsFeedEvent（消耗 self）
    ///
    /// 仅处理 ORDER_TRADE_UPDATE（合约订单更新）事件。
    /// ACCOUNT_UPDATE 等非订单事件返回 None，由调用方处理。
    pub fn to_ws_feed_event(self) -> Option<WsFeedEvent> {
        // 单流 ORDER_TRADE_UPDATE（合约）
        if let Some(et) = self.event_type_flat.as_deref() {
            if et == "ORDER_TRADE_UPDATE" {
                // ORDER_TRADE_UPDATE 的订单数据在 "o" 字段
                if let Some(order) = self.order_flat {
                    return order.to_ws_feed_event();
                }
            }
        }
        // 组合流 ORDER_TRADE_UPDATE（合约）
        if let Some(data) = self.data {
            if data.event_type == "ORDER_TRADE_UPDATE" {
                return data.order.to_ws_feed_event();
            }
        }
        None
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct BinanceOrderData {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: i64,
    #[serde(rename = "o")]
    pub order: BinanceOrderInner,
}

/// Binance ORDER_TRADE_UPDATE 中的订单数据
/// 文档: https://binance-docs.github.io/apidocs/futures/en/#event-order-update
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct BinanceOrderInner {
    /// 订单符号
    #[serde(rename = "s")]
    pub(crate) symbol: String,
    /// 客户端订单 ID
    #[serde(rename = "c")]
    pub(crate) client_order_id: String,
    /// 侧: BUY / SELL
    #[serde(rename = "S")]
    pub(crate) side: String,
    /// 订单类型
    #[serde(rename = "o")]
    pub(crate) order_type: String,
    /// 订单状态
    #[serde(rename = "X")]
    pub(crate) status: String,
    /// 订单 ID
    #[serde(rename = "i")]
    pub(crate) order_id: i64,
    /// 原始订单数量
    #[serde(rename = "q")]
    pub(crate) orig_qty: String,
    /// 已填充数量
    #[serde(rename = "z")]
    pub(crate) filled_qty: String,
    /// 剩余数量
    #[serde(rename = "Q")]
    pub(crate) remaining_qty: Option<String>,
    /// 成交价格（最后一笔成交价）
    #[serde(rename = "L")]
    pub(crate) last_fill_price: String,
    /// 累计成交均价（仅永续合约 ORDER_TRADE_UPDATE 提供）
    #[serde(rename = "ap")]
    pub(crate) avg_fill_price: Option<String>,
    /// 成交数量
    #[serde(rename = "l")]
    pub(crate) last_fill_qty: String,
    /// 手续费
    #[serde(rename = "n")]
    pub(crate) commission: String,
    /// 手续费资产
    #[serde(rename = "N")]
    pub(crate) commission_asset: String,
    /// 订单创建时间
    #[serde(rename = "T")]
    pub(crate) trade_time: i64,
    /// 是否是 reduce-only
    #[serde(rename = "R")]
    pub(crate) is_reduce_only: bool,
    /// 工作类型
    #[serde(rename = "w")]
    pub(crate) working_type: String,
    /// 持仓方向: LONG / SHORT / BOTH（双向持仓模式下区分多空持仓）
    #[serde(rename = "ps")]
    pub(crate) position_side: Option<String>,
}

impl BinanceOrderInner {
    /// 将 Binance 订单状态映射为 Position Engine 的 OrderStatus
    pub(crate) fn to_order_status(&self) -> Option<OrderStatus> {
        match self.status.as_str() {
            "NEW" => Some(OrderStatus::Open),
            "PARTIALLY_FILLED" => Some(OrderStatus::PartiallyFilled),
            "FILLED" => Some(OrderStatus::Filled),
            "CANCELED" => Some(OrderStatus::Canceled),
            "EXPIRED" => Some(OrderStatus::Canceled),
            "EXPIRED_IN_MATCH" => Some(OrderStatus::Canceled),
            "REJECTED" => Some(OrderStatus::Failed),
            _ => None,
        }
    }

    /// 转换为 WsFeedEvent::OrderUpdate
    ///
    /// 关键数值字段（filled/amount/price/commission）解析失败时返回 None，
    /// 跳过该事件而非传播 0.0，避免订单状态判断错误和 PnL 计算偏差。
    pub fn to_ws_feed_event(&self) -> Option<WsFeedEvent> {
        let status = self.to_order_status()?;

        let position_side = self
            .position_side
            .as_ref()
            .and_then(|ps| match ps.as_str() {
                "LONG" => Some(PositionSide::Long),
                "SHORT" => Some(PositionSide::Short),
                _ => None,
            });

        let filled = self.filled_qty.parse::<f64>().unwrap_or_else(|e| {
            tracing::error!(
                filled_qty = %self.filled_qty,
                error = %e,
                "Failed to parse filled_qty in order_ws — skipping event to avoid 0.0 propagation"
            );
            f64::NAN
        });
        if filled.is_nan() {
            return None;
        }

        let amount = self.orig_qty.parse::<f64>().unwrap_or_else(|e| {
            tracing::error!(
                orig_qty = %self.orig_qty,
                error = %e,
                "Failed to parse orig_qty in order_ws — skipping event to avoid 0.0 propagation"
            );
            f64::NAN
        });
        if amount.is_nan() {
            return None;
        }

        let remaining = self
            .remaining_qty
            .as_ref()
            .and_then(|q| q.parse().ok())
            .unwrap_or_else(|| (amount - filled).max(0.0));

        let price = self
            .avg_fill_price
            .as_ref()
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|&p| p > 0.0)
            .unwrap_or_else(|| {
                match self.last_fill_price.parse::<f64>() {
                    Ok(p) if p > 0.0 => p,
                    Ok(_) => {
                        tracing::warn!(
                            last_fill_price = %self.last_fill_price,
                            symbol = %self.symbol,
                            "last_fill_price is 0.0 in order_ws — using 0.0 (order may not be filled yet)"
                        );
                        0.0
                    }
                    Err(e) => {
                        tracing::error!(
                            last_fill_price = %self.last_fill_price,
                            error = %e,
                            "Failed to parse last_fill_price in order_ws — skipping event to avoid 0.0 price propagation"
                        );
                        return f64::NAN;
                    }
                }
            });
        if price.is_nan() {
            return None;
        }

        let commission = match self.commission.parse::<f64>() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(
                    commission = %self.commission,
                    error = %e,
                    "Failed to parse commission in order_ws — skipping event to avoid 0.0 propagation"
                );
                return None;
            }
        };

        Some(WsFeedEvent::OrderUpdate {
            exchange_order_id: self.order_id.to_string(),
            client_order_id: Some(self.client_order_id.clone()),
            symbol: self.symbol.clone(),
            status,
            filled,
            remaining,
            price,
            amount,
            commission,
            timestamp: DateTime::from_timestamp_millis(self.trade_time).unwrap_or_else(|| {
                tracing::warn!(
                    trade_time = self.trade_time,
                    symbol = %self.symbol,
                    order_id = %self.order_id,
                    "WS order trade_time invalid — using local time as fallback"
                );
                Utc::now()
            }),
            position_side,
        })
    }
}

// ============================================================
// UserDataWsHandler: WsHandler 实现
// ============================================================

/// WS 消息延迟告警阈值（毫秒）
///
/// 订单事件比 kline 更关键（影响资金状态），阈值设为 3 秒。
/// 延迟来源可能是网络传输、channel 堆积或客户端处理阻塞。
pub(crate) const ORDER_WS_DELAY_THRESHOLD_MS: i64 = 3_000;

/// Binance User Data Stream 的 [`WsHandler`] 实现
///
/// 通过 `refresh_url()` 在每次重连前重新创建 listenKey，解决 P0 问题：
/// 原始实现中 listenKey 过期后用旧 key 重连导致死循环。
///
/// `current_key` 通过 `Arc<RwLock<String>>` 与外部 keepalive task 共享，
/// 确保 `refresh_url()` 创建新 key 后，keepalive task 能续期正确的 key。
///
/// 消息解析委托给 [`dispatch_event`]，统一处理 11 种事件类型。
/// `listenKeyExpired` 事件触发 `MessageOutcome::Reconnect`，
/// WsManager 立即断开并通过 `refresh_url()` 获取新 key。
pub struct UserDataWsHandler {
    /// 初始 WS URL（含初始 listenKey）
    ws_url: String,
    /// HTTP 客户端 — 用于 `refresh_url()` 创建新 listenKey
    client: ExchangeClient,
    /// 签名器 — 用于 `refresh_url()` 签名
    signer: Arc<dyn Signer>,
    /// 共享 listenKey — keepalive task 和 `refresh_url()` 通过此字段同步
    current_key: Arc<RwLock<String>>,
}

impl UserDataWsHandler {
    /// 创建 handler
    ///
    /// `ws_url` 应包含初始 listenKey，格式：`wss://fstream.binance.com/private/ws?listenKey=<key>`
    ///
    /// `current_key` 是共享 listenKey 句柄，`refresh_url()` 更新后 keepalive task 可读取最新值。
    pub fn new(
        ws_url: String,
        client: ExchangeClient,
        signer: Arc<dyn Signer>,
        current_key: Arc<RwLock<String>>,
    ) -> Self {
        Self {
            ws_url,
            client,
            signer,
            current_key,
        }
    }
}

#[async_trait::async_trait]
impl WsHandler<WsFeedEvent> for UserDataWsHandler {
    fn base_url(&self) -> &str {
        &self.ws_url
    }

    async fn refresh_url(&self) -> Result<String, ExchangeError> {
        // P0 修复：每次重连前创建新 listenKey，而非复用旧 key
        let new_key = fapi::create_listen_key(&self.client, self.signer.as_ref()).await?;
        // 更新共享 listenKey，使 keepalive task 续期新 key 而非已失效的旧 key
        *self.current_key
            .write()
            .expect("listenKey RwLock poisoned") = new_key.clone();
        let url = format!("wss://fstream.binance.com/private/ws?listenKey={}", new_key);
        tracing::info!("[UserDataWs] Refreshed listenKey for reconnect");
        Ok(url)
    }

    async fn on_message(
        &self,
        text: &str,
    ) -> Result<MessageOutcome<WsFeedEvent>, ExchangeError> {
        // 延迟检测：解析 event_time 并与本地时间对比
        if let Ok(bmsg) = serde_json::from_str::<BinanceOrderMessage>(text) {
            if let Some(et) = bmsg.event_time() {
                if et > 0 {
                    let delay_ms = chrono::Utc::now().timestamp_millis() - et;
                    if delay_ms > ORDER_WS_DELAY_THRESHOLD_MS {
                        let event_type = bmsg
                            .event_type()
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "unknown".to_string());
                        tracing::warn!(
                            delay_ms = delay_ms,
                            event_time = et,
                            event_type = %event_type,
                            "[UserDataWs] Order event delay exceeds threshold"
                        );
                    }
                }
            }
        }

        // 委托给 dispatch_event 解析 — 支持 11 种事件类型
        if let Some(event) = dispatch_event(text) {
            return Ok(MessageOutcome::Continue(vec![event]));
        }

        // dispatch_event 返回 None 时，检查是否为 listenKeyExpired
        // listen_key_expired::process() 返回 None 但记录了 error 日志
        // 我们需要检测该事件类型并触发重连
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
            let payload = value.get("data").unwrap_or(&value);
            if let Some(et) = payload.get("e").and_then(|v| v.as_str()) {
                if et == "listenKeyExpired" {
                    tracing::warn!(
                        "[UserDataWs] listenKey expired — requesting reconnect with fresh key"
                    );
                    return Ok(MessageOutcome::Reconnect);
                }
                if et == "serverShutdown" {
                    tracing::warn!("[UserDataWs] Server shutdown event — requesting reconnect");
                    return Ok(MessageOutcome::Reconnect);
                }
            }
        }

        // 非订单事件（ACCOUNT_UPDATE / MARGIN_CALL 等）— dispatch_event 已处理日志
        Ok(MessageOutcome::Continue(vec![]))
    }

    async fn on_connected(&self, _is_reconnect: bool) -> Vec<String> {
        // UserDataWs 是单流设计，无需发送 SUBSCRIBE 消息
        vec![]
    }

    async fn on_disconnected(&self) {
        // 无需清理 — 订阅状态由 WsManager 管理
    }
}

// ============================================================
// UserDataWs: 委托给 WsManager 的薄包装
// ============================================================

/// Binance User Data Stream 订单推送客户端
///
/// 内部委托给 [`WsManager<WsFeedEvent>`]，仅保留对外 API 兼容性。
///
/// 改进（对比原始实现）：
/// - **P0 修复**：`refresh_url()` 每次重连前创建新 listenKey
/// - **连接超时**：`connect_timeout_secs` 防止 `connect_async` 挂起
/// - **Pong 超时**：`pong_timeout_secs` 检测半开连接
/// - **优雅关闭**：统一 `stop()` + shutdown channel
/// - **熔断**：`max_retries` 超限后触发 `CircuitBreakerTripped` 事件
/// - **listenKeyExpired**：`MessageOutcome::Reconnect` 立即触发重连
/// - **listenKey 共享**：`Arc<RwLock<String>>` 使 keepalive task 与 `refresh_url()` 同步
pub struct UserDataWs {
    manager: WsManager<WsFeedEvent>,
    /// 初始 WS URL — 保留用于测试断言
    pub ws_url: String,
    /// 共享 listenKey — keepalive task 通过 `listen_key_handle()` 读取最新值
    current_key: Arc<RwLock<String>>,
}

impl UserDataWs {
    /// 创建永续合约订单 WS 客户端
    ///
    /// 2026-04-23 起币安将用户数据流切流至 /private 路由，
    /// 新 URL 使用 query 形态 `wss://fstream.binance.com/private/ws?listenKey=<key>`
    ///
    /// `client` 和 `signer` 用于 `refresh_url()` 在每次重连前创建新 listenKey。
    /// 内部创建 `Arc<RwLock<String>>` 共享 listenKey，使 keepalive task 能读取
    /// `refresh_url()` 更新后的最新 key。
    pub fn new_perpetual(
        listen_key: String,
        client: ExchangeClient,
        signer: Arc<dyn Signer>,
    ) -> Self {
        let base_url = "wss://fstream.binance.com/private/ws".to_string();
        let ws_url = format!("{}?listenKey={}", base_url, listen_key);

        let current_key = Arc::new(RwLock::new(listen_key));

        let handler = Arc::new(UserDataWsHandler::new(
            ws_url.clone(),
            client,
            signer,
            Arc::clone(&current_key),
        ));

        let config = WsManagerConfig::default();

        Self {
            manager: WsManager::new(config, handler),
            ws_url,
            current_key,
        }
    }

    /// 返回 running flag 的引用，供外部 keepalive task 检测 WS 生命周期。
    pub fn running_handle(&self) -> std::sync::Arc<std::sync::atomic::AtomicBool> {
        self.manager.running_handle()
    }

    /// 返回共享 listenKey 的句柄，供外部 keepalive task 读取最新值。
    ///
    /// `refresh_url()` 在每次重连前创建新 listenKey 并更新此共享状态，
    /// keepalive task 每次 tick 时从此处读取当前 listenKey 进行 PUT 续期。
    pub fn listen_key_handle(&self) -> Arc<RwLock<String>> {
        Arc::clone(&self.current_key)
    }

    /// 启动 WS 连接，将事件发送到 event_tx
    ///
    /// 内部通过 forwarder task 将 `WsManagerEvent<WsFeedEvent>` 转换为 `WsFeedEvent`：
    /// - `Message(e)` → `e`
    /// - `ConnectionChanged { connected, .. }` → `WsFeedEvent::ConnectionChanged { connected }`
    /// - `CircuitBreakerTripped` → `WsFeedEvent::ConnectionChanged { connected: false }` + 日志
    pub async fn start(&self, event_tx: mpsc::Sender<WsFeedEvent>) {
        // WsManager 发出 WsManagerEvent<WsFeedEvent>，需要桥接到 WsFeedEvent
        let (manager_tx, mut manager_rx) = mpsc::channel::<WsManagerEvent<WsFeedEvent>>(256);

        // 启动 WsManager
        self.manager.start(manager_tx).await;

        // forwarder task: WsManagerEvent<WsFeedEvent> → WsFeedEvent
        tokio::spawn(async move {
            while let Some(ev) = manager_rx.recv().await {
                let feed_event = match ev {
                    WsManagerEvent::Message(e) => e,
                    WsManagerEvent::ConnectionChanged { connected, .. } => {
                        WsFeedEvent::ConnectionChanged { connected }
                    }
                    WsManagerEvent::CircuitBreakerTripped { retry_count } => {
                        tracing::error!(
                            retry_count = retry_count,
                            "[UserDataWs] Circuit breaker tripped — WS stopped after max retries"
                        );
                        WsFeedEvent::ConnectionChanged { connected: false }
                    }
                };
                if event_tx.send(feed_event).await.is_err() {
                    tracing::warn!("[UserDataWs] External event channel closed, stopping forwarder");
                    break;
                }
            }
        });
    }

    /// 优雅关闭
    pub async fn stop(&self) {
        self.manager.stop().await;
    }
}
