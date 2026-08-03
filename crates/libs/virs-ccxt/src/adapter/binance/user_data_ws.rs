use std::sync::{Arc, RwLock};

use serde::Deserialize;
use tokio::sync::mpsc;

use virs_types::WsFeedEvent;

use crate::adapter::binance::fapi;
use crate::adapter::binance::user_data_ws_events::dispatch_event;
use crate::auth::Signer;
use virs_ws::{MessageOutcome, WsHandler, WsManager, WsManagerConfig, WsManagerEvent};
use crate::ExchangeClient;
use virs_runtime::TaskSupervisor;

// Binance用户数据WebSocket消息，兼容两种格式：
// 组合流: {"stream":...,"data":{"e":"...",...}}
// 扁平流: {"e":"...","E":...,"o":{...}}
// 注意: 订单数据解析由 dispatch_event → OrderTradeUpdateData 负责（user_data_ws_events/），
//       本结构仅用于延迟检测（event_type/event_time），不再解析订单字段。
#[derive(Debug, Clone, Deserialize)]
pub struct BinanceOrderMessage {
    pub(crate) data: Option<BinanceOrderData>, // 组合流内层事件数据

    #[serde(rename = "e")]
    pub(crate) event_type_flat: Option<String>, // 扁平流的事件类型

    #[serde(rename = "E")]
    event_time_flat: Option<i64>, // 扁平流的事件时间(ms)
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
}

// 组合流内层事件数据（仅保留延迟检测所需字段）
#[derive(Debug, Clone, Deserialize)]
pub struct BinanceOrderData {
    #[serde(rename = "e")]
    pub event_type: String, // e→事件类型
    #[serde(rename = "E")]
    pub event_time: i64, // E→事件时间(ms)
}

// 延迟阈值: 事件时间超过本地时间3秒视为延迟
pub(crate) const ORDER_WS_DELAY_THRESHOLD_MS: i64 = 3_000;

// 用户数据WebSocket处理器，管理listenKey和消息分发
pub struct UserDataWsHandler {
    ws_url: String, // WebSocket连接URL

    client: ExchangeClient, // 交易所客户端(用于创建listenKey)

    signer: Arc<dyn Signer>, // 签名器

    current_key: Arc<RwLock<String>>, // 当前listenKey(共享给重连逻辑)
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
    async fn refresh_url(&self) -> Result<String, virs_error::VirsError> {
        // 调用fapi创建新的listenKey
        let new_key = fapi::create_listen_key(&self.client, self.signer.as_ref()).await?;

        // 更新共享的current_key
        *self.current_key.write().expect("listenKey RwLock poisoned") = new_key.clone();
        let url = format!("wss://fstream.binance.com/private/ws?listenKey={}", new_key);
        tracing::info!("Refreshed listenKey for reconnect");
        Ok(url)
    }

    // 收到消息: 先做延迟检测(阈值3秒)，再dispatch_event分发，最后检查listenKeyExpired和serverShutdown
    async fn on_message(&self, text: &str) -> Result<MessageOutcome<WsFeedEvent>, virs_error::VirsError> {
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
                            "Order event delay exceeds threshold"
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
                        "listenKey expired — requesting reconnect with fresh key"
                    );
                    return Ok(MessageOutcome::Reconnect);
                }
                if et == "serverShutdown" {
                    tracing::warn!("Server shutdown event — requesting reconnect");
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

    async fn on_disconnected(&self) {}
}

// 用户数据WebSocket封装，管理连接生命周期和事件转发
pub struct UserDataWs {
    manager: WsManager<WsFeedEvent>,

    config: WsManagerConfig,

    pub ws_url: String, // 连接URL: wss://fstream.binance.com/private/ws?listenKey=xxx

    current_key: Arc<RwLock<String>>, // 当前listenKey

    /// 转发任务监督器 — 管理转发任务的 JoinHandle + 取消信号
    supervisor: TaskSupervisor,
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

        Self {
            manager: WsManager::new(handler),
            config: WsManagerConfig::default(),
            ws_url,
            current_key,
            supervisor: TaskSupervisor::new(virs_runtime::CancellationToken::root()),
        }
    }

    // 获取运行状态句柄
    pub fn running_handle(&self) -> std::sync::Arc<std::sync::atomic::AtomicBool> {
        self.manager.running_handle()
    }

    /// 获取 CancellationToken（如果正在运行）
    pub async fn cancellation_token(&self) -> Option<virs_runtime::CancellationToken> {
        self.manager.cancellation_token().await
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
        self.manager
            .start(self.config.clone(), manager_tx)
            .await;

        // 获取 WsManager 的取消令牌，用于转发任务响应 WS 关闭
        let ws_cancel = self
            .manager
            .cancellation_token()
            .await
            .unwrap_or_else(|| virs_runtime::CancellationToken::root());

        // 转发任务: 将WsManagerEvent转为WsFeedEvent发送到外部channel
        // 通过 TaskSupervisor 管理 JoinHandle + 取消信号
        self.supervisor
            .spawn_raw("user_data_forward", move |supervisor_cancel| async move {
                loop {
                    tokio::select! {
                        _ = supervisor_cancel.cancelled() => break,
                        _ = ws_cancel.cancelled() => break,
                        ev = manager_rx.recv() => {
                            let Some(ev) = ev else { break };
                            let feed_event = match ev {
                                WsManagerEvent::Message(e) => e,
                                WsManagerEvent::ConnectionChanged { connected, .. } => {
                                    WsFeedEvent::ConnectionChanged { connected }
                                }
                                WsManagerEvent::CircuitBreakerTripped { retry_count } => {
                                    tracing::error!(
                                        retry_count = retry_count,
                                        "Circuit breaker tripped — WS stopped after max retries"
                                    );
                                    WsFeedEvent::ConnectionChanged { connected: false }
                                }
                            };
                            if event_tx.send(feed_event).await.is_err() {
                                tracing::warn!(
                                    "External event channel closed, stopping forwarder"
                                );
                                break;
                            }
                        }
                    }
                }
            })
            .await;
    }

    // 停止WS
    pub async fn stop(&self) {
        self.manager.stop().await;
        self.supervisor.shutdown().await;
    }
}
