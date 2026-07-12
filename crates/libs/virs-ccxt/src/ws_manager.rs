//! WsManager — 公共 WebSocket 连接管理组件
//!
//! 封装连接管理、心跳、重连和错误处理的全部逻辑，
//! 业务 WS 客户端通过实现 [`WsHandler`] trait 只关注消息解析和业务逻辑。
//!
//! ## 使用方式
//!
//! ```ignore
//! let handler = Arc::new(MyWsHandler::new(...));
//! let manager = WsManager::new(WsManagerConfig::default(), handler);
//! let (tx, rx) = mpsc::channel(256);
//! manager.start(tx).await;
//! // manager.stop().await; // 优雅关闭
//! ```

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{connect_async, tungstenite};
use virs_error::ExchangeError;

// ============================================================
// 命令与事件类型
// ============================================================

/// WS 动态订阅命令
///
/// 通过 [`WsManager::send_command`] 发送，由 [`WsHandler::on_command`] 处理。
#[derive(Debug, Clone)]
pub enum WsCommand {
    Subscribe(String),
    Unsubscribe(String),
}

/// `on_message` 处理结果
///
/// - `Continue`：转发事件，WS 继续运行
/// - `Reconnect`：请求强制重连（如 listenKeyExpired），WsManager 立即断开
///   当前连接并通过 `refresh_url()` 获取新 URL 后重连
#[derive(Debug, Clone)]
pub enum MessageOutcome<T: Send + Clone + 'static> {
    /// 正常处理 — events 通过 [`WsManagerEvent::Message`] 转发给消费者
    Continue(Vec<T>),
    /// 请求强制重连 — 立即断开当前连接，通过 `refresh_url()` 获取新 URL
    Reconnect,
}

/// [`WsManager`] 对外发出的事件
///
/// 泛型参数 `T` 是业务消息类型（如 `WsEvent`、`WsOrderBookEvent`、`WsFeedEvent`）。
#[derive(Debug, Clone)]
pub enum WsManagerEvent<T: Send + Clone + 'static> {
    /// 业务消息（由 [`WsHandler::on_message`] 解析后返回）
    Message(T),

    /// 连接状态变化
    ///
    /// - `connected=true, is_reconnect=false`：首次连接成功
    /// - `connected=true, is_reconnect=true`：重连成功
    /// - `connected=false`：连接断开（即将自动重连）
    ConnectionChanged {
        connected: bool,
        is_reconnect: bool,
    },

    /// 熔断触发 — `max_retries` 已达上限
    ///
    /// 收到此事件后 [`WsManager`] 已停止，不会自动重连。
    /// 消费者应执行降级策略（如切换数据源、通知用户、暂停交易）。
    CircuitBreakerTripped { retry_count: u64 },
}

// ============================================================
// WS 连接参数常量
// ============================================================

/// Ping 心跳间隔（秒）— 定时发送 WebSocket Ping 帧
pub const WS_PING_INTERVAL_SECS: u64 = 30;

/// Pong 超时阈值（秒）— 超过此时间未收到任何消息则认为连接已死
pub const WS_PONG_TIMEOUT_SECS: u64 = 90;

/// 连接超时（秒）— `connect_async` 的 timeout 包裹
pub const WS_CONNECT_TIMEOUT_SECS: u64 = 10;

/// 重连初始延迟（秒）— 首次重连等待时间
pub const WS_RECONNECT_INITIAL_DELAY_SECS: u64 = 1;

/// 重连最大延迟（秒）— 指数退避上限
pub const WS_RECONNECT_MAX_DELAY_SECS: u64 = 60;

/// 连接最大生命周期（秒）— 到期后主动断开重连
///
/// 币安 WS 连接 24 小时后会被服务器强制断开，设为 23 小时提前重连。
pub const WS_MAX_LIFETIME_SECS: u64 = 82_800; // 23h

/// 最大重试次数 — 超过后触发熔断 [`WsManagerEvent::CircuitBreakerTripped`]
pub const WS_MAX_RETRIES: u64 = 100;

// ============================================================
// 配置
// ============================================================

/// [`WsManager`] 配置
///
/// 所有参数均使用模块级常量，通过 `Default` 构造。
/// 如需调整，修改 [`ws_manager`] 模块中的常量并重新编译。
#[derive(Debug, Clone)]
pub struct WsManagerConfig {
    ping_interval_secs: u64,
    pong_timeout_secs: u64,
    connect_timeout_secs: u64,
    reconnect_initial_delay_secs: u64,
    reconnect_max_delay_secs: u64,
    max_lifetime_secs: u64,
    max_retries: u64,
}

impl WsManagerConfig {
    /// 仅供测试：覆盖 pong_timeout_secs
    #[cfg(test)]
    pub fn with_pong_timeout(mut self, secs: u64) -> Self {
        self.pong_timeout_secs = secs;
        self
    }

    /// 仅供测试：覆盖 max_retries
    #[cfg(test)]
    pub fn with_max_retries(mut self, n: u64) -> Self {
        self.max_retries = n;
        self
    }

    /// 仅供测试：覆盖 connect_timeout_secs
    #[cfg(test)]
    pub fn with_connect_timeout(mut self, secs: u64) -> Self {
        self.connect_timeout_secs = secs;
        self
    }
}

impl Default for WsManagerConfig {
    fn default() -> Self {
        Self {
            ping_interval_secs: WS_PING_INTERVAL_SECS,
            pong_timeout_secs: WS_PONG_TIMEOUT_SECS,
            connect_timeout_secs: WS_CONNECT_TIMEOUT_SECS,
            reconnect_initial_delay_secs: WS_RECONNECT_INITIAL_DELAY_SECS,
            reconnect_max_delay_secs: WS_RECONNECT_MAX_DELAY_SECS,
            max_lifetime_secs: WS_MAX_LIFETIME_SECS,
            max_retries: WS_MAX_RETRIES,
        }
    }
}

// ============================================================
// WsHandler Trait
// ============================================================

/// 业务 WS 处理器 trait
///
/// [`WsManager`] 在连接生命周期的各阶段调用对应方法，
/// 业务客户端只需关注消息解析和订阅状态管理。
///
/// ## 方法调用时序
///
/// ```text
/// start()
///   └─ loop {
///        refresh_url()          ← 每次连接/重连前
///        connect_async (timeout)
///        ├─ on_connected()      ← 连接成功后（返回订阅消息）
///        │   └─ select! loop:
///        │        on_message()  ← 收到文本消息
///        │        on_command()  ← 收到订阅命令
///        │        (ping tick)
///        │        (shutdown)
///        ├─ on_disconnected()   ← 连接断开后
///        └─ backoff + retry
///      }
/// ```
#[async_trait]
pub trait WsHandler<T: Send + Clone + 'static>: Send + Sync {
    /// 返回基础 URL（用于首次连接）
    ///
    /// 重连时通过 [`Self::refresh_url`] 获取最新 URL。
    fn base_url(&self) -> &str;

    /// 是否支持动态订阅命令
    ///
    /// 返回 `true` 时，[`WsManager`] 会创建 command channel，
    /// 消费者可通过 [`WsManager::send_command`] 发送订阅命令。
    fn supports_commands(&self) -> bool {
        false
    }

    /// 刷新连接 URL — 每次重连前调用
    ///
    /// UserDataWs 在此重新创建 listenKey 并返回新 URL。
    /// 默认实现返回 [`Self::base_url`]（公共流无需刷新）。
    /// 如果刷新失败，[`WsManager`] 会跳过本次连接并进入退避重试。
    async fn refresh_url(&self) -> Result<String, ExchangeError> {
        Ok(self.base_url().to_string())
    }

    /// 处理收到的文本消息，返回处理结果
    ///
    /// - `Ok(Continue(events))`：events 转发给消费者，WS 继续运行
    /// - `Ok(Reconnect)`：请求强制重连，WsManager 立即断开并通过 `refresh_url()` 获取新 URL
    /// - `Err(e)`：记录 warn 日志但不断连（背压容忍）
    async fn on_message(&self, text: &str) -> Result<MessageOutcome<T>, ExchangeError>;

    /// 连接成功后调用 — 返回需要发送给交易所的消息（如 SUBSCRIBE JSON）
    ///
    /// - `is_reconnect=false`：首次连接
    /// - `is_reconnect=true`：重连，应返回所有已有订阅的 SUBSCRIBE 消息以恢复状态
    ///
    /// 返回的每条字符串作为一个独立的 WebSocket Text 帧发送。
    async fn on_connected(&self, is_reconnect: bool) -> Vec<String>;

    /// 连接断开时调用 — 用于清理运行时状态
    ///
    /// 订阅列表等持久状态不应在此清理（重连后需要恢复订阅）。
    async fn on_disconnected(&self);

    /// 处理订阅命令 — 仅在 [`Self::supports_commands`] 返回 true 时调用
    ///
    /// 返回 `Some(msg)`：msg 作为 WebSocket Text 帧发送给交易所
    /// 返回 `None`：不发送任何消息
    async fn on_command(&self, _cmd: WsCommand) -> Option<String> {
        None
    }
}

// ============================================================
// WsManager
// ============================================================

/// 公共 WebSocket 连接管理器
///
/// 封装连接生命周期、心跳、重连和错误处理的全部逻辑。
/// 业务 WS 客户端通过 [`WsHandler`] trait 注入消息解析逻辑。
///
/// ## 状态机
///
/// ```text
/// Idle ──start()──→ Connecting ──ok──→ Connected ──error──→ Reconnecting ──backoff──→ Connecting
///                       │                   │                     │
///                    timeout/error       stop()/lifetime       stop()/max_retries
///                       ↓                   ↓                     ↓
///                   Reconnecting        Disconnected         Disconnected (CircuitBreaker)
/// ```
///
/// ## 线程安全
///
/// 内部使用 `Arc` + `AtomicBool` + `Mutex`，可以安全地 clone 和跨 task 共享。
/// `start()` 和 `stop()` 可在不同 task 中调用。
pub struct WsManager<T: Send + Clone + 'static> {
    config: WsManagerConfig,
    handler: Arc<dyn WsHandler<T>>,
    running: Arc<AtomicBool>,
    retry_count: Arc<AtomicU64>,
    shutdown_tx: Mutex<Option<mpsc::Sender<()>>>,
    command_tx: Mutex<Option<mpsc::UnboundedSender<WsCommand>>>,
}

impl<T: Send + Clone + 'static> WsManager<T> {
    /// 创建新的 WsManager，创建后需调用 [`Self::start`] 启动后台连接 task
    pub fn new(config: WsManagerConfig, handler: Arc<dyn WsHandler<T>>) -> Self {
        Self {
            config,
            handler,
            running: Arc::new(AtomicBool::new(false)),
            retry_count: Arc::new(AtomicU64::new(0)),
            shutdown_tx: Mutex::new(None),
            command_tx: Mutex::new(None),
        }
    }

    /// 返回 running flag 的引用，供外部检测 WS 是否仍在运行
    pub fn running_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.running)
    }

    /// 启动 WS 连接 — 后台 tokio task 中运行，立即返回
    ///
    /// 事件通过 `event_tx` 发送给消费者：
    /// - [`WsManagerEvent::ConnectionChanged`]：连接/断开通知
    /// - [`WsManagerEvent::Message`]：业务消息
    /// - [`WsManagerEvent::CircuitBreakerTripped`]：熔断通知
    ///
    /// 重复调用是安全的（running flag 保护）。
    pub async fn start(&self, event_tx: mpsc::Sender<WsManagerEvent<T>>) {
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        *self.shutdown_tx.lock().await = Some(shutdown_tx);

        let mut command_rx = if self.handler.supports_commands() {
            let (tx, rx) = mpsc::unbounded_channel::<WsCommand>();
            *self.command_tx.lock().await = Some(tx);
            Some(rx)
        } else {
            None
        };

        let config = self.config.clone();
        let handler = Arc::clone(&self.handler);
        let running = Arc::clone(&self.running);
        let retry_count = Arc::clone(&self.retry_count);

        tokio::spawn(async move {
            let mut reconnect_delay = config.reconnect_initial_delay_secs;
            let mut is_first_connect = true;

            while running.load(Ordering::Relaxed) {
                let connect_start = tokio::time::Instant::now();

                // ── 1. 刷新 URL ──────────────────────────────────────
                let ws_url = match handler.refresh_url().await {
                    Ok(url) => url,
                    Err(e) => {
                        tracing::error!(
                            error = %e,
                            "[WsManager] refresh_url failed — will retry after backoff"
                        );
                        if !Self::do_backoff(
                            &running,
                            &retry_count,
                            &config,
                            &mut reconnect_delay,
                            &event_tx,
                        )
                        .await
                        {
                            break;
                        }
                        continue;
                    }
                };

                // ── 2. 带超时的连接 ──────────────────────────────────
                let connect_result = tokio::time::timeout(
                    Duration::from_secs(config.connect_timeout_secs),
                    connect_async(&ws_url),
                )
                .await;

                match connect_result {
                    Ok(Ok((ws_stream, _))) => {
                        reconnect_delay = config.reconnect_initial_delay_secs;
                        retry_count.store(0, Ordering::Relaxed);

                        let is_reconnect = !is_first_connect;
                        if event_tx
                            .send(WsManagerEvent::ConnectionChanged {
                                connected: true,
                                is_reconnect,
                            })
                            .await
                            .is_err()
                        {
                            tracing::warn!(
                                "[WsManager] Event channel closed on connect, stopping"
                            );
                            running.store(false, Ordering::Relaxed);
                            break;
                        }
                        is_first_connect = false;

                        let (mut write, mut read) = ws_stream.split();

                        // ── 3. 调用 on_connected — 恢复订阅状态 ──────────
                        let connect_msgs = handler.on_connected(is_reconnect).await;
                        let mut write_ok = true;
                        for msg in &connect_msgs {
                            if write
                                .send(tungstenite::Message::Text(msg.clone().into()))
                                .await
                                .is_err()
                            {
                                tracing::warn!(
                                    "[WsManager] Failed to send on_connected message, reconnecting"
                                );
                                write_ok = false;
                                break;
                            }
                        }

                        if write_ok {
                            let ping_interval = Duration::from_secs(config.ping_interval_secs);
                            let mut ping_tick = tokio::time::interval(ping_interval);
                            let max_lifetime = Duration::from_secs(config.max_lifetime_secs);
                            let mut last_msg_time = tokio::time::Instant::now();

                        // ── 4. 内层 select! 循环 ─────────────────────────
                        loop {
                            if !running.load(Ordering::Relaxed) {
                                break;
                            }

                            // 连接最大生命周期检查
                            if connect_start.elapsed() > max_lifetime {
                                tracing::info!(
                                    "[WsManager] Max lifetime ({}s) reached, reconnecting",
                                    config.max_lifetime_secs
                                );
                                break;
                            }

                            // Pong 超时检测 — 无消息超时则强制重连
                            if last_msg_time.elapsed()
                                > Duration::from_secs(config.pong_timeout_secs)
                            {
                                tracing::warn!(
                                    pong_timeout_secs = config.pong_timeout_secs,
                                    "[WsManager] No message received within pong timeout, forcing reconnect"
                                );
                                break;
                            }

                            tokio::select! {
                                msg = read.next() => {
                                    last_msg_time = tokio::time::Instant::now();

                                    match msg {
                                        Some(Ok(tungstenite::Message::Text(text))) => {
                                            match handler.on_message(&text).await {
                                                Ok(MessageOutcome::Continue(events)) => {
                                                    for ev in events {
                                                        if event_tx.send(WsManagerEvent::Message(ev)).await.is_err() {
                                                            tracing::warn!(
                                                                "[WsManager] Event channel closed — stopping WS"
                                                            );
                                                            running.store(false, Ordering::Relaxed);
                                                            break;
                                                        }
                                                    }
                                                }
                                                Ok(MessageOutcome::Reconnect) => {
                                                    tracing::info!(
                                                        "[WsManager] Handler requested reconnect"
                                                    );
                                                    break;
                                                }
                                                Err(e) => {
                                                    tracing::warn!(
                                                        error = %e,
                                                        msg_preview = &text[..text.len().min(200)],
                                                        "[WsManager] on_message error — skipping, WS continues"
                                                    );
                                                }
                                            }
                                        }
                                        Some(Ok(tungstenite::Message::Ping(data))) => {
                                            let _ = write.send(tungstenite::Message::Pong(data)).await;
                                        }
                                        Some(Ok(tungstenite::Message::Pong(_))) => {}
                                        Some(Ok(tungstenite::Message::Close(_))) => {
                                            tracing::warn!("[WsManager] Server closed connection");
                                            break;
                                        }
                                        Some(Ok(tungstenite::Message::Binary(_))) => {}
                                        Some(Ok(tungstenite::Message::Frame(_))) => {}
                                        Some(Err(e)) => {
                                            tracing::error!(error = %e, "[WsManager] Read error");
                                            break;
                                        }
                                        None => {
                                            tracing::warn!("[WsManager] Stream ended");
                                            break;
                                        }
                                    }
                                }
                                _ = ping_tick.tick() => {
                                    let ping = tungstenite::Message::Ping(vec![].into());
                                    if write.send(ping).await.is_err() {
                                        tracing::warn!("[WsManager] Ping failed, reconnecting");
                                        break;
                                    }
                                }
                                cmd = async {
                                    match &mut command_rx {
                                        Some(rx) => rx.recv().await,
                                        None => std::future::pending().await
                                    }
                                } => {
                                    if let Some(cmd) = cmd {
                                        if let Some(msg) = handler.on_command(cmd).await {
                                            if write
                                                .send(tungstenite::Message::Text(msg.into()))
                                                .await
                                                .is_err()
                                            {
                                                tracing::warn!(
                                                    "[WsManager] Command send failed, reconnecting"
                                                );
                                                break;
                                            }
                                        }
                                    } else {
                                        break;
                                    }
                                }
                                _ = shutdown_rx.recv() => {
                                    tracing::info!("[WsManager] Shutdown signal received, closing");
                                    let _ = write.send(tungstenite::Message::Close(None)).await;
                                    running.store(false, Ordering::Relaxed);
                                    handler.on_disconnected().await;
                                    return;
                                }
                            }
                        }
                        } // end if write_ok

                        // ── 5. 连接断开处理 ──────────────────────────────
                        handler.on_disconnected().await;

                        if event_tx
                            .send(WsManagerEvent::ConnectionChanged {
                                connected: false,
                                is_reconnect: false,
                            })
                            .await
                            .is_err()
                        {
                            tracing::warn!(
                                "[WsManager] Event channel closed on disconnect, stopping"
                            );
                            running.store(false, Ordering::Relaxed);
                            break;
                        }
                    }
                    Ok(Err(e)) => {
                        tracing::error!(error = %e, "[WsManager] Connection failed");
                    }
                    Err(_) => {
                        tracing::error!(
                            timeout_secs = config.connect_timeout_secs,
                            "[WsManager] Connection timeout"
                        );
                    }
                }

                // ── 6. 退避 + 重试检查 ────────────────────────────────
                if !Self::do_backoff(
                    &running,
                    &retry_count,
                    &config,
                    &mut reconnect_delay,
                    &event_tx,
                )
                .await
                {
                    break;
                }
            }

            running.store(false, Ordering::Relaxed);
        });
    }

    /// 优雅关闭 — 发送 shutdown 信号，后台 task 发送 Close 帧后退出
    pub async fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(tx) = self.shutdown_tx.lock().await.take() {
            let _ = tx.send(()).await;
        }
    }

    /// 发送订阅命令（仅 `supports_commands()=true` 的 handler 有效）
    pub async fn send_command(&self, cmd: WsCommand) {
        if let Some(tx) = self.command_tx.lock().await.as_ref() {
            let _ = tx.send(cmd);
        }
    }

    /// 当前是否正在运行
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// 当前重试次数
    pub fn retry_count(&self) -> u64 {
        self.retry_count.load(Ordering::Relaxed)
    }

    // ── 内部辅助方法 ──────────────────────────────────────────

    /// 执行退避等待 + 重试计数 + 熔断检查
    ///
    /// 返回 `true` 表示可以继续重试，`false` 表示应停止（running=false 或熔断触发）。
    async fn do_backoff(
        running: &Arc<AtomicBool>,
        retry_count: &Arc<AtomicU64>,
        config: &WsManagerConfig,
        reconnect_delay: &mut u64,
        event_tx: &mpsc::Sender<WsManagerEvent<T>>,
    ) -> bool {
        if !running.load(Ordering::Relaxed) {
            return false;
        }

        // 熔断检查
        if config.max_retries > 0 {
            let retries = retry_count.fetch_add(1, Ordering::Relaxed) + 1;
            if retries >= config.max_retries {
                tracing::error!(
                    retries = retries,
                    max_retries = config.max_retries,
                    "[WsManager] Max retries exceeded — circuit breaker tripped"
                );
                let _ = event_tx
                    .send(WsManagerEvent::CircuitBreakerTripped { retry_count: retries })
                    .await;
                return false;
            }
        }

        // 指数退避 + jitter
        let jitter = rand::random::<f64>() * *reconnect_delay as f64 * 0.2;
        let delay = *reconnect_delay as f64 + jitter;
        tokio::time::sleep(Duration::from_secs_f64(delay)).await;

        *reconnect_delay = (*reconnect_delay * 2).min(config.reconnect_max_delay_secs);

        running.load(Ordering::Relaxed)
    }
}
