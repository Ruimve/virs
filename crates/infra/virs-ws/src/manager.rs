use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{connect_async, tungstenite};
use virs_error::VirsError;
use virs_task::{spawn, Stop, TaskHandle};

/* WebSocket 连接管理常量：控制心跳、重连、熔断等关键参数 */
pub const WS_PING_INTERVAL_SECS: u64 = 30;
pub const WS_PONG_TIMEOUT_SECS: u64 = 90;
pub const WS_CONNECT_TIMEOUT_SECS: u64 = 10;
/* 重连初始延迟：1 秒，后续指数退避加倍 */
pub const WS_RECONNECT_INITIAL_DELAY_SECS: u64 = 1;
/* 重连最大延迟：60 秒，指数退避上限 */
pub const WS_RECONNECT_MAX_DELAY_SECS: u64 = 60;
/* 连接最大生命周期：约 23 小时，超过后主动重连防止交易所端连接老化 */
pub const WS_MAX_LIFETIME_SECS: u64 = 82_800;
/* 最大重试次数：超过后触发熔断器，停止重连 */
pub const WS_MAX_RETRIES: u64 = 100;

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

/* 连接状态原因：区分首次连接、重连、断开重连和主动停止 */
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionReason {
    Connected,
    Reconnected,
    DisconnectedReconnecting,
    Stopped,
}

/* WebSocket 管理器事件：消息、连接状态变化和熔断器触发 */
#[derive(Debug, Clone)]
pub enum WsManagerEvent<T: Send + Clone + 'static> {
    Message(T),

    ConnectionChanged {
        connected: bool,
        reason: ConnectionReason,
    },

    /* 熔断器触发：重试次数超过上限时通知上层 */
    CircuitBreakerTripped { retry_count: u64 },
}

#[derive(Debug, Clone)]
pub enum WsCommand {
    Subscribe(String),
    Unsubscribe(String),
}

/* 消息处理结果：Continue 表示继续处理并产生事件，Reconnect 表示需要强制重连 */
#[derive(Debug, Clone)]
pub enum MessageOutcome<T: Send + Clone + 'static> {
    Continue(Vec<T>),
    Reconnect,
}

/* 退避结果：Proceed 继续/重连，CircuitBroken 熔断，Shutdown 关闭 */
enum BackoffOutcome {
    Proceed,
    CircuitBroken,
    Shutdown,
}

/*
 * WebSocket 处理器 trait：由具体业务实现，定义 URL 刷新、消息解析、连接/断开回调和命令处理。
 * on_message 返回 MessageOutcome，可指示继续处理或强制重连。
 */
#[async_trait]
pub trait WsHandler<T: Send + Clone + 'static>: Send + Sync {
    fn base_url(&self) -> &str;

    fn supports_commands(&self) -> bool {
        false
    }

    async fn refresh_url(&self) -> Result<String, VirsError> {
        Ok(self.base_url().to_string())
    }

    async fn on_message(&self, text: &str) -> Result<MessageOutcome<T>, VirsError>;

    async fn on_binary(&self, _data: &[u8]) -> Result<MessageOutcome<T>, VirsError> {
        Ok(MessageOutcome::Continue(vec![]))
    }

    async fn on_connected(&self, is_reconnect: bool) -> Vec<String>;

    async fn on_disconnected(&self);

    async fn on_command(&self, _cmd: WsCommand) -> Option<String> {
        None
    }
}

/*
 * WebSocket 管理器：封装连接、重连、心跳、熔断和命令处理的完整生命周期。
 * 通过 virs_task::spawn 启动后台任务，使用 Stop 令牌实现优雅关闭。
 */
pub struct WsManager<T: Send + Clone + 'static> {
    handler: Arc<dyn WsHandler<T>>,
    running: Arc<AtomicBool>,
    retry_count: Arc<AtomicU64>,
    task: std::sync::Mutex<Option<TaskHandle>>,
    command_tx: Mutex<Option<mpsc::UnboundedSender<WsCommand>>>,
}

impl<T: Send + Clone + 'static> WsManager<T> {
    pub fn new(handler: Arc<dyn WsHandler<T>>) -> Self {
        Self {
            handler,
            running: Arc::new(AtomicBool::new(false)),
            retry_count: Arc::new(AtomicU64::new(0)),
            task: std::sync::Mutex::new(None),
            command_tx: Mutex::new(None),
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

    /*
     * 启动 WebSocket 管理器：使用 CAS 保证只启动一次，通过 virs_task::spawn 创建后台任务。
     * 后台任务循环执行：连接→消息处理→断开→指数退避重连，直到收到取消信号或熔断。
     */
    pub async fn start(
        &self,
        config: WsManagerConfig,
        event_tx: mpsc::Sender<WsManagerEvent<T>>,
    ) {
        /* CAS 保证 start 只执行一次，已运行时直接返回 */
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        self.retry_count.store(0, Ordering::Relaxed);

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

        let handle = spawn("ws_main", move |stop| async move {
            /* 重连延迟从初始值开始，每次失败后指数加倍直到最大值 */
            let mut reconnect_delay = config.reconnect_initial_delay_secs;
            let mut is_first_connect = true;

            while !stop.is_cancelled() {
                let connect_start = tokio::time::Instant::now();

                let ws_url = match handler.refresh_url().await {
                    Ok(url) => url,
                    Err(e) => {
                        tracing::error!(error = %e, "refresh_url failed");
                        match backoff_with_cancel(
                            &stop,
                            &retry_count,
                            &config,
                            &mut reconnect_delay,
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
                        /* 连接成功：重置重连延迟和重试计数 */
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
                            handler.on_disconnected().await;
                            running.store(false, Ordering::Relaxed);
                            break;
                        }
                        let is_reconnect = !is_first_connect;
                        is_first_connect = false;

                        let (mut write, mut read) = ws_stream.split();

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
                            /* 心跳定时器：首次 ping 在一个间隔后触发 */
                            let ping_start = tokio::time::Instant::now()
                                + Duration::from_secs(config.ping_interval_secs);
                            let mut ping_tick = tokio::time::interval_at(
                                ping_start,
                                Duration::from_secs(config.ping_interval_secs),
                            );
                            let max_lifetime = Duration::from_secs(config.max_lifetime_secs);
                            let mut last_msg_time = tokio::time::Instant::now();

                            /* 主事件循环：处理消息、ping、命令和取消信号 */
                            loop {
                                if stop.is_cancelled() {
                                    break;
                                }

                                /* 连接生命周期上限：超过后主动断开重连 */
                                if connect_start.elapsed() > max_lifetime {
                                    tracing::info!("Max lifetime reached, reconnecting");
                                    break;
                                }

                                /* Pong 超时：超过阈值未收到消息，强制重连 */
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
                                                        let preview: String = text.chars().take(200).collect();
                                                        tracing::warn!(error = %e, msg_preview = %preview, "on_message error — skipping");
                                                    }
                                                }
                                            }
                                            Some(Ok(tungstenite::Message::Binary(data))) => {
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
                                    _ = stop.cancelled() => {
                                        /* 收到取消信号：发送 Close 帧并优雅退出 */
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

                        handler.on_disconnected().await;
                        if event_tx
                            .send(WsManagerEvent::ConnectionChanged {
                                connected: false,
                                reason: ConnectionReason::DisconnectedReconnecting,
                            })
                            .await
                            .is_err()
                        {
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

                match backoff_with_cancel(
                    &stop,
                    &retry_count,
                    &config,
                    &mut reconnect_delay,
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

            send_stopped(&event_tx, &running).await;
        });

        *self.task.lock().unwrap() = Some(handle);
    }

    /* 停止 WebSocket 管理器：取消任务并等待优雅关闭（使用默认超时） */
    pub async fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        let handle = self.task.lock().unwrap().take();
        if let Some(h) = handle {
            h.cancel();
            h.join().await;
        }
    }

    pub async fn send_command(&self, cmd: WsCommand) {
        if let Some(tx) = self.command_tx.lock().await.as_ref() {
            let _ = tx.send(cmd);
        }
    }
}

/*
 * 指数退避重连逻辑：检查取消信号→重试计数→带抖动的指数退避等待。
 * 抖动量为延迟的 20%，避免多个客户端同时重连导致的惊群效应。
 * 重试次数超过上限时触发熔断器，返回 CircuitBroken。
 */
async fn backoff_with_cancel<T: Send + Clone + 'static>(
    stop: &Stop,
    retry_count: &AtomicU64,
    config: &WsManagerConfig,
    reconnect_delay: &mut u64,
    event_tx: &mpsc::Sender<WsManagerEvent<T>>,
) -> BackoffOutcome {
    if stop.is_cancelled() {
        return BackoffOutcome::Shutdown;
    }

    if config.max_retries > 0 {
        let retries = retry_count.fetch_add(1, Ordering::Relaxed) + 1;
        /* 重试次数达到上限：触发熔断器 */
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

    /* 抖动：随机 0~20% 的延迟，避免惊群效应 */
    let jitter = rand::random::<f64>() * *reconnect_delay as f64 * 0.2;
    let delay = *reconnect_delay as f64 + jitter;

    tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs_f64(delay)) => {
            /* 退避后延迟加倍，但不超过最大值 */
            *reconnect_delay = (*reconnect_delay * 2).min(config.reconnect_max_delay_secs);
            BackoffOutcome::Proceed
        }
        _ = stop.cancelled() => {
            BackoffOutcome::Shutdown
        }
    }
}

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
