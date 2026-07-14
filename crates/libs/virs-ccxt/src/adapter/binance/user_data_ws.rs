use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use serde::Deserialize;
use tokio::sync::mpsc;
use virs_error::ExchangeError;


pub use virs_types::WsFeedEvent;
use virs_types::{OrderStatus, PositionSide};

use crate::adapter::binance::fapi;
use crate::adapter::binance::user_data_ws_events::dispatch_event;
use crate::auth::Signer;
use crate::ws_manager::{MessageOutcome, WsHandler, WsManager, WsManagerConfig, WsManagerEvent};
use crate::ExchangeClient;


// Binance用户数据WebSocket消息，兼容两种格式：
// 组合流: {"stream":...,"data":{"e":"...","o":{...}}}
// 扁平流: {"e":"...","E":...,"o":{...}}
#[derive(Debug, Clone, Deserialize)]
pub struct BinanceOrderMessage {
    #[allow(dead_code)]
    pub(crate) stream: Option<String>,  // 组合流的stream名称（扁平流无此字段）

    pub(crate) data: Option<BinanceOrderData>,  // 组合流内层事件数据

    #[serde(rename = "e")]
    pub(crate) event_type_flat: Option<String>,  // 扁平流的事件类型

    #[serde(rename = "E")]
    event_time_flat: Option<i64>,  // 扁平流的事件时间(ms)
    #[serde(rename = "o")]
    order_flat: Option<BinanceOrderInner>,  // 扁平流的订单数据
}

impl BinanceOrderMessage {
    // 获取事件类型，优先扁平流，回退组合流
    pub fn event_type(&self) -> Option<&str> {
        self.event_type_flat
            .as_deref()
            .or_else(|| self.data.as_ref().map(|d| d.event_type.as_str()))
    }

    // 获取事件时间，优先扁平流，回退组合流
    pub fn event_time(&self) -> Option<i64> {
        self.event_time_flat
            .or_else(|| self.data.as_ref().map(|d| d.event_time))
    }

    // 转换为WsFeedEvent，先尝试扁平流解析，再尝试组合流
    pub fn to_ws_feed_event(self) -> Option<WsFeedEvent> {
        // 扁平流格式解析
        if let Some(et) = self.event_type_flat.as_deref() {
            if et == "ORDER_TRADE_UPDATE" {

                if let Some(order) = self.order_flat {
                    return order.to_ws_feed_event();
                }
            }
        }

        // 组合流格式解析
        if let Some(data) = self.data {
            if data.event_type == "ORDER_TRADE_UPDATE" {
                return data.order.to_ws_feed_event();
            }
        }
        None
    }
}

// 组合流内层事件数据
#[derive(Debug, Clone, Deserialize)]
pub struct BinanceOrderData {
    #[serde(rename = "e")]
    pub event_type: String,  // e→事件类型
    #[serde(rename = "E")]
    pub event_time: i64,  // E→事件时间(ms)
    #[serde(rename = "o")]
    pub order: BinanceOrderInner,  // o→订单详情
}


// ORDER_TRADE_UPDATE事件内层订单数据
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct BinanceOrderInner {

    #[serde(rename = "s")]
    pub(crate) symbol: String,  // s→交易对

    #[serde(rename = "c")]
    pub(crate) client_order_id: String,  // c→客户端订单ID

    #[serde(rename = "S")]
    pub(crate) side: String,  // S→买卖方向

    #[serde(rename = "o")]
    pub(crate) order_type: String,  // o→订单类型

    #[serde(rename = "X")]
    pub(crate) status: String,  // X→订单状态

    #[serde(rename = "i")]
    pub(crate) order_id: i64,  // i→交易所订单ID

    #[serde(rename = "q")]
    pub(crate) orig_qty: String,  // q→原始数量

    #[serde(rename = "z")]
    pub(crate) filled_qty: String,  // z→已成交数量

    #[serde(rename = "Q")]
    pub(crate) remaining_qty: Option<String>,  // Q→剩余未成交数量

    #[serde(rename = "L")]
    pub(crate) last_fill_price: String,  // L→最新成交价

    #[serde(rename = "ap")]
    pub(crate) avg_fill_price: Option<String>,  // ap→平均成交价

    #[serde(rename = "l")]
    pub(crate) last_fill_qty: String,  // l→最新成交数量

    #[serde(rename = "n")]
    pub(crate) commission: String,  // n→手续费

    #[serde(rename = "N")]
    pub(crate) commission_asset: String,  // N→手续费资产

    #[serde(rename = "T")]
    pub(crate) trade_time: i64,  // T→成交时间(ms)

    #[serde(rename = "R")]
    pub(crate) is_reduce_only: bool,  // R→是否仅减仓

    #[serde(rename = "w")]
    pub(crate) working_type: String,  // w→工作类型(逐仓/全仓)

    #[serde(rename = "ps")]
    pub(crate) position_side: Option<String>,  // ps→持仓方向
}

impl BinanceOrderInner {
    // 订单状态映射: NEW→Open, PARTIALLY_FILLED→PartiallyFilled, FILLED→Filled,
    // CANCELED/EXPIRED/EXPIRED_IN_MATCH→Canceled, REJECTED→Failed
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

    // 转换为WsFeedEvent::OrderUpdate
    pub fn to_ws_feed_event(&self) -> Option<WsFeedEvent> {
        let status = self.to_order_status()?;

        // 解析持仓方向 LONG/SHORT
        let position_side = self
            .position_side
            .as_ref()
            .and_then(|ps| match ps.as_str() {
                "LONG" => Some(PositionSide::Long),
                "SHORT" => Some(PositionSide::Short),
                _ => None,
            });

        // 解析已成交数量，失败则跳过事件
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

        // 解析原始订单数量，失败则跳过事件
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

        // 剩余数量: 优先用交易所返回值，否则用 orig_qty - filled_qty
        let remaining = self
            .remaining_qty
            .as_ref()
            .and_then(|q| q.parse().ok())
            .unwrap_or_else(|| (amount - filled).max(0.0));

        // 成交价: 优先用avg_fill_price，回退last_fill_price
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

        // 手续费，解析失败则跳过事件
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


// 延迟阈值: 事件时间超过本地时间3秒视为延迟
pub(crate) const ORDER_WS_DELAY_THRESHOLD_MS: i64 = 3_000;


// 用户数据WebSocket处理器，管理listenKey和消息分发
pub struct UserDataWsHandler {

    ws_url: String,  // WebSocket连接URL

    client: ExchangeClient,  // 交易所客户端(用于创建listenKey)

    signer: Arc<dyn Signer>,  // 签名器

    current_key: Arc<RwLock<String>>,  // 当前listenKey(共享给重连逻辑)
}

impl UserDataWsHandler {

    // 构造处理器
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

    // 重连时重新创建listenKey，确保不过期
    async fn refresh_url(&self) -> Result<String, ExchangeError> {
        // 调用fapi创建新的listenKey
        let new_key = fapi::create_listen_key(&self.client, self.signer.as_ref()).await?;

        // 更新共享的current_key
        *self.current_key
            .write()
            .expect("listenKey RwLock poisoned") = new_key.clone();
        let url = format!("wss://fstream.binance.com/private/ws?listenKey={}", new_key);
        tracing::info!("[UserDataWs] Refreshed listenKey for reconnect");
        Ok(url)
    }

    // 收到消息: 先做延迟检测(阈值3秒)，再dispatch_event分发，最后检查listenKeyExpired和serverShutdown
    async fn on_message(
        &self,
        text: &str,
    ) -> Result<MessageOutcome<WsFeedEvent>, ExchangeError> {
        // 延迟检测: 比较事件时间与本地时间
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

        // 事件分发
        if let Some(event) = dispatch_event(text) {
            return Ok(MessageOutcome::Continue(vec![event]));
        }

        // 检查listenKey过期和服务器关闭事件，触发重连
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

        // 其他消息忽略
        Ok(MessageOutcome::Continue(vec![]))
    }

    // 连接成功回调，无需发送订阅消息(listenKey已在URL中)
    async fn on_connected(&self, _is_reconnect: bool) -> Vec<String> {
        vec![]
    }

    async fn on_disconnected(&self) {

    }
}


// 用户数据WebSocket封装，管理连接生命周期和事件转发
pub struct UserDataWs {
    manager: WsManager<WsFeedEvent>,  // WS连接管理器

    pub ws_url: String,  // 连接URL: wss://fstream.binance.com/private/ws?listenKey=xxx

    current_key: Arc<RwLock<String>>,  // 当前listenKey
}

impl UserDataWs {

    // 构造永续合约用户数据WS，URL格式: wss://fstream.binance.com/private/ws?listenKey=xxx
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

    // 获取运行状态句柄
    pub fn running_handle(&self) -> std::sync::Arc<std::sync::atomic::AtomicBool> {
        self.manager.running_handle()
    }

    // 获取listenKey句柄
    pub fn listen_key_handle(&self) -> Arc<RwLock<String>> {
        Arc::clone(&self.current_key)
    }

    // 启动WS，转发WsManagerEvent为WsFeedEvent
    pub async fn start(&self, event_tx: mpsc::Sender<WsFeedEvent>) {
        // 创建内部channel连接WsManager和转发任务
        let (manager_tx, mut manager_rx) = mpsc::channel::<WsManagerEvent<WsFeedEvent>>(256);

        // 启动WsManager
        self.manager.start(manager_tx).await;

        // 转发任务: 将WsManagerEvent转为WsFeedEvent发送到外部channel
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

    // 停止WS
    pub async fn stop(&self) {
        self.manager.stop().await;
    }
}
