use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use tokio_tungstenite::{connect_async, tungstenite};
use virs_error::VirsError;

// ── 公共常量 ──

/// PING 间隔：30秒
pub const WS_PING_INTERVAL_SECS: u64 = 30;
/// PONG 超时：90秒无消息则强制重连
pub const WS_PONG_TIMEOUT_SECS: u64 = 90;
/// 连接超时：10秒
pub const WS_CONNECT_TIMEOUT_SECS: u64 = 10;
/// 重连初始退避：1秒
pub const WS_RECONNECT_INITIAL_DELAY_SECS: u64 = 1;
/// 重连最大退避：60秒
pub const WS_RECONNECT_MAX_DELAY_SECS: u64 = 60;
/// 连接最大存活：23小时（82800秒），到期主动重连
pub const WS_MAX_LIFETIME_SECS: u64 = 82_800;
/// 最大重连次数：超过则熔断
pub const WS_MAX_RETRIES: u64 = 100;

// ── 配置 ──

#[derive(Debug, Clone)]
pub struct WsManagerConfig {
    pub ping_interval_secs: u64,
    pub pong_timeout_secs: u64,
    pub connect_timeout_secs: u64,
    pub reconnect_initial_delay_secs: u64,
    pub reconnect_max_delay_secs: u64,
    pub max_lifetime_secs: u64,
    pub max_retries: u64,
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

// ── 事件与命令 ──

/// 连接状态变化原因
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionReason {
    /// 首次连接成功
    Connected,
    /// 重连成功
    Reconnected,
    /// 临时断连，即将自动重连
    DisconnectedReconnecting,
    /// 主动停止（stop() 或 shutdown）
    Stopped,
}

/// WS管理器事件
#[derive(Debug, Clone)]
pub enum WsManagerEvent<T: Send + Clone + 'static> {
    /// 消息事件
    Message(T),

    /// 连接状态变化
    ConnectionChanged {
        connected: bool,
        reason: ConnectionReason,
    },

    /// 熔断触发
    CircuitBreakerTripped { retry_count: u64 },
}

/// 动态订阅/退订命令
#[derive(Debug, Clone)]
pub enum WsCommand {
    Subscribe(String),
    Unsubscribe(String),
}

/// 消息处理结果
#[derive(Debug, Clone)]
pub enum MessageOutcome<T: Send + Clone + 'static> {
    /// 继续运行并产出事件
    Continue(Vec<T>),
    /// 请求重连
    Reconnect,
}

/// 退避结果
enum BackoffOutcome {
    /// 退避结束，可以重连
    Proceed,
    /// 熔断
    CircuitBroken,
    /// 收到 shutdown
    Shutdown,
}

// ── Handler trait ──

#[async_trait]
pub trait WsHandler<T: Send + Clone + 'static>: Send + Sync {
    /// 返回连接 URL
    fn base_url(&self) -> &str;

    /// 是否支持动态订阅/退订命令，默认 false
    fn supports_commands(&self) -> bool {
        false
    }

    /// 重连时刷新 URL（如 listenKey 场景需重新获取），默认返回 base_url
    async fn refresh_url(&self) -> Result<String, VirsError> {
        Ok(self.base_url().to_string())
    }

    /// 解析 Text 消息
    async fn on_message(&self, text: &str) -> Result<MessageOutcome<T>, VirsError>;

    /// 解析 Binary 消息，默认返回空事件列表
    async fn on_binary(&self, _data: &[u8]) -> Result<MessageOutcome<T>, VirsError> {
        Ok(MessageOutcome::Continue(vec![]))
    }

    /// 连接后发送的初始订阅消息列表
    async fn on_connected(&self, is_reconnect: bool) -> Vec<String>;

    /// 断连回调
    async fn on_disconnected(&self);

    /// 动态订阅命令转 JSON 文本，不支持时返回 None
    async fn on_command(&self, _cmd: WsCommand) -> Option<String> {
        None
    }
}

// ── WsManager ──

pub struct WsManager<T: Send + Clone + 'static> {
    handler: Arc<dyn WsHandler<T>>,
    running: Arc<AtomicBool>,
    retry_count: Arc<AtomicU64>,
    shutdown_tx: Mutex<Option<mpsc::Sender<()>>>,
    command_tx: Mutex<Option<mpsc::UnboundedSender<WsCommand>>>,
    task_handle: Mutex<Option<JoinHandle<()>>>,
}

impl<T: Send + Clone + 'static> WsManager<T> {
    pub fn new(handler: Arc<dyn WsHandler<T>>) -> Self {
        Self {
            handler,
            running: Arc::new(AtomicBool::new(false)),
            retry_count: Arc::new(AtomicU64::new(0)),
            shutdown_tx: Mutex::new(None),
            command_tx: Mutex::new(None),
            task_handle: Mutex::new(None),
        }
    }

    pub fn running_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.running)
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn retry_count(&self) -> u64 {
        self.retry_count.load(Ordering::Relaxed)
    }

    pub async fn start(
        &self,
        config: WsManagerConfig,
        event_tx: mpsc::Sender<WsManagerEvent<T>>,
    ) {
        // 防止重复启动
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        // D3 修复：每次 start 重置 retry_count
        self.retry_count.store(0, Ordering::Relaxed);

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        *self.shutdown_tx.lock().await = Some(shutdown_tx);

        let mut command_rx = if self.handler.supports_commands() {
            let (tx, rx) = mpsc::unbounded_channel::<WsCommand>();
            *self.command_tx.lock().await = Some(tx);
            Some(rx)
        } else {
            None
        };

        let handler = Arc::clone(&self.handler);
        let running = Arc::clone(&self.running);
        let retry_count = Arc::clone(&self.retry_count);

        let handle = tokio::spawn(async move {
            let mut reconnect_delay = config.reconnect_initial_delay_secs;
            let mut is_first_connect = true;

            while running.load(Ordering::Relaxed) {
                let connect_start = tokio::time::Instant::now();

                // ── Connecting 阶段 ──

                let ws_url = match handler.refresh_url().await {
                    Ok(url) => url,
                    Err(e) => {
                        tracing::error!(error = %e, "refresh_url failed");
                        match backoff_with_shutdown(
                            &running,
                            &retry_count,
                            &config,
                            &mut reconnect_delay,
                            &mut shutdown_rx,
                            &event_tx,
                        )
                        .await
                        {
                            BackoffOutcome::Proceed => continue,
                            BackoffOutcome::CircuitBroken => break,
                            BackoffOutcome::Shutdown => {
                                send_stopped(&event_tx, &running).await;
                                return;
                            }
                        }
                    }
                };

                let connect_result = tokio::time::timeout(
                    Duration::from_secs(config.connect_timeout_secs),
                    connect_async(&ws_url),
                )
                .await;

                match connect_result {
                    Ok(Ok((ws_stream, _))) => {
                        // 连接成功：重置退避与重试计数
                        reconnect_delay = config.reconnect_initial_delay_secs;
                        retry_count.store(0, Ordering::Relaxed);

                        let reason = if is_first_connect {
                            ConnectionReason::Connected
                        } else {
                            ConnectionReason::Reconnected
                        };
                        if event_tx
                            .send(WsManagerEvent::ConnectionChanged {
                                connected: true,
                                reason,
                            })
                            .await
                            .is_err()
                        {
                            // 消费者已断开：连接已建立，需调用 on_disconnected 清理
                            handler.on_disconnected().await;
                            running.store(false, Ordering::Relaxed);
                            break;
                        }
                        let is_reconnect = !is_first_connect;
                        is_first_connect = false;

                        // ── Connected 阶段 ──

                        let (mut write, mut read) = ws_stream.split();

                        // 发送初始订阅消息
                        let connect_msgs = handler.on_connected(is_reconnect).await;
                        let mut write_ok = true;
                        for msg in &connect_msgs {
                            if write
                                .send(tungstenite::Message::Text(msg.clone().into()))
                                .await
                                .is_err()
                            {
                                write_ok = false;
                                break;
                            }
                        }

                        if write_ok {
                            // D8 修复：延迟首次 Ping 到一个完整间隔后
                            let ping_start =
                                tokio::time::Instant::now() + Duration::from_secs(config.ping_interval_secs);
                            let mut ping_tick = tokio::time::interval_at(
                                ping_start,
                                Duration::from_secs(config.ping_interval_secs),
                            );
                            let max_lifetime = Duration::from_secs(config.max_lifetime_secs);
                            let mut last_msg_time = tokio::time::Instant::now();

                            loop {
                                if !running.load(Ordering::Relaxed) {
                                    break;
                                }

                                if connect_start.elapsed() > max_lifetime {
                                    tracing::info!("Max lifetime reached, reconnecting");
                                    break;
                                }

                                if last_msg_time.elapsed()
                                    > Duration::from_secs(config.pong_timeout_secs)
                                {
                                    tracing::warn!("Pong timeout, forcing reconnect");
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
                                                                running.store(false, Ordering::Relaxed);
                                                                break;
                                                            }
                                                        }
                                                    }
                                                    Ok(MessageOutcome::Reconnect) => {
                                                        break;
                                                    }
                                                    Err(e) => {
                                                        // D7 修复：安全的 msg_preview
                                                        let preview: String = text.chars().take(200).collect();
                                                        tracing::warn!(error = %e, msg_preview = %preview, "on_message error — skipping");
                                                    }
                                                }
                                            }
                                            Some(Ok(tungstenite::Message::Binary(data))) => {
                                                // D9 修复：Binary 消息交给 handler
                                                match handler.on_binary(&data).await {
                                                    Ok(MessageOutcome::Continue(events)) => {
                                                        for ev in events {
                                                            if event_tx.send(WsManagerEvent::Message(ev)).await.is_err() {
                                                                running.store(false, Ordering::Relaxed);
                                                                break;
                                                            }
                                                        }
                                                    }
                                                    Ok(MessageOutcome::Reconnect) => break,
                                                    Err(e) => {
                                                        tracing::warn!(error = %e, "on_binary error — skipping");
                                                    }
                                                }
                                            }
                                            Some(Ok(tungstenite::Message::Ping(data))) => {
                                                let _ = write.send(tungstenite::Message::Pong(data)).await;
                                            }
                                            Some(Ok(tungstenite::Message::Pong(_))) => {}
                                            Some(Ok(tungstenite::Message::Close(_))) => {
                                                tracing::warn!("Server closed connection");
                                                break;
                                            }
                                            Some(Ok(tungstenite::Message::Frame(_))) => {}
                                            Some(Err(e)) => {
                                                tracing::error!(error = %e, "Read error");
                                                break;
                                            }
                                            None => {
                                                tracing::warn!("Stream ended");
                                                break;
                                            }
                                        }
                                    }
                                    _ = ping_tick.tick() => {
                                        if write.send(tungstenite::Message::Ping(vec![].into())).await.is_err() {
                                            break;
                                        }
                                    }
                                    cmd = async {
                                        match &mut command_rx {
                                            Some(rx) => rx.recv().await,
                                            None => std::future::pending().await,
                                        }
                                    } => {
                                        if let Some(cmd) = cmd {
                                            if let Some(msg) = handler.on_command(cmd).await {
                                                if write.send(tungstenite::Message::Text(msg.into())).await.is_err() {
                                                    break;
                                                }
                                            }
                                        } else {
                                            break;
                                        }
                                    }
                                    _ = shutdown_rx.recv() => {
                                        // D5 修复：shutdown 路径也发送断连事件
                                        tracing::info!("Shutdown signal received, closing");
                                        let _ = write.send(tungstenite::Message::Close(None)).await;
                                        handler.on_disconnected().await;
                                        let _ = event_tx.send(WsManagerEvent::ConnectionChanged {
                                            connected: false,
                                            reason: ConnectionReason::Stopped,
                                        }).await;
                                        running.store(false, Ordering::Relaxed);
                                        return;
                                    }
                                }
                            }
                        }

                        // 正常断连路径
                        handler.on_disconnected().await;
                        if event_tx
                            .send(WsManagerEvent::ConnectionChanged {
                                connected: false,
                                reason: ConnectionReason::DisconnectedReconnecting,
                            })
                            .await
                            .is_err()
                        {
                            // 消费者已断开：无需重连，直接退出
                            running.store(false, Ordering::Relaxed);
                            break;
                        }
                    }
                    Ok(Err(e)) => {
                        tracing::error!(error = %e, "Connection failed");
                    }
                    Err(_) => {
                        tracing::error!("Connection timeout");
                    }
                }

                // ── BackingOff 阶段 ──

                match backoff_with_shutdown(
                    &running,
                    &retry_count,
                    &config,
                    &mut reconnect_delay,
                    &mut shutdown_rx,
                    &event_tx,
                )
                .await
                {
                    BackoffOutcome::Proceed => continue,
                    BackoffOutcome::CircuitBroken => break,
                    BackoffOutcome::Shutdown => {
                        send_stopped(&event_tx, &running).await;
                        return;
                    }
                }
            }

            // while 循环退出唯一路径：CircuitBroken（Shutdown 已 return）
            send_stopped(&event_tx, &running).await;
        });

        // D2 修复：存储 JoinHandle 供 stop() 等待
        *self.task_handle.lock().await = Some(handle);
    }

    pub async fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(tx) = self.shutdown_tx.lock().await.take() {
            let _ = tx.send(()).await;
        }
        // D2 修复：等待 task 真正结束
        if let Some(handle) = self.task_handle.lock().await.take() {
            let _ = handle.await;
        }
    }

    pub async fn send_command(&self, cmd: WsCommand) {
        if let Some(tx) = self.command_tx.lock().await.as_ref() {
            let _ = tx.send(cmd);
        }
    }
}

// ── 自由函数 ──

/// D10 修复：退避+熔断+shutdown 感知的自由函数
async fn backoff_with_shutdown<T: Send + Clone + 'static>(
    running: &AtomicBool,
    retry_count: &AtomicU64,
    config: &WsManagerConfig,
    reconnect_delay: &mut u64,
    shutdown_rx: &mut mpsc::Receiver<()>,
    event_tx: &mpsc::Sender<WsManagerEvent<T>>,
) -> BackoffOutcome {
    if !running.load(Ordering::Relaxed) {
        return BackoffOutcome::Shutdown;
    }

    // 熔断检查
    if config.max_retries > 0 {
        let retries = retry_count.fetch_add(1, Ordering::Relaxed) + 1;
        if retries >= config.max_retries {
            tracing::error!(
                retries = retries,
                max_retries = config.max_retries,
                "Circuit breaker tripped"
            );
            if event_tx
                .send(WsManagerEvent::CircuitBreakerTripped {
                    retry_count: retries,
                })
                .await
                .is_err()
            {
                return BackoffOutcome::Shutdown;
            }
            return BackoffOutcome::CircuitBroken;
        }
    }

    // D1 修复：退避 sleep 期间响应 shutdown
    let jitter = rand::random::<f64>() * *reconnect_delay as f64 * 0.2;
    let delay = *reconnect_delay as f64 + jitter;

    tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs_f64(delay)) => {
            *reconnect_delay = (*reconnect_delay * 2).min(config.reconnect_max_delay_secs);
            BackoffOutcome::Proceed
        }
        _ = shutdown_rx.recv() => {
            BackoffOutcome::Shutdown
        }
    }
}

/// 统一发送 Stopped 事件并标记 running=false。
/// 不调用 on_disconnected()——该回调由实际持有连接的路径负责调用。
async fn send_stopped<T: Send + Clone + 'static>(
    event_tx: &mpsc::Sender<WsManagerEvent<T>>,
    running: &AtomicBool,
) {
    let _ = event_tx
        .send(WsManagerEvent::ConnectionChanged {
            connected: false,
            reason: ConnectionReason::Stopped,
        })
        .await;
    running.store(false, Ordering::Relaxed);
}
