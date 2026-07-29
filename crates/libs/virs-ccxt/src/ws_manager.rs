use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{connect_async, tungstenite};
use virs_error::ExchangeError;

// 动态订阅/退订命令
#[derive(Debug, Clone)]
pub enum WsCommand {
    Subscribe(String),
    Unsubscribe(String),
}

// 消息处理结果：Continue 继续运行并产出事件，Reconnect 请求重连
#[derive(Debug, Clone)]
pub enum MessageOutcome<T: Send + Clone + 'static> {
    Continue(Vec<T>),

    Reconnect,
}

// WS管理器事件：Message 消息、ConnectionChanged 连接状态变化、CircuitBreakerTripped 熔断
#[derive(Debug, Clone)]
pub enum WsManagerEvent<T: Send + Clone + 'static> {
    Message(T),

    ConnectionChanged { connected: bool, is_reconnect: bool },

    CircuitBreakerTripped { retry_count: u64 },
}

// PING 间隔：30秒
pub const WS_PING_INTERVAL_SECS: u64 = 30;

// PONG 超时：90秒无消息则强制重连
pub const WS_PONG_TIMEOUT_SECS: u64 = 90;

// 连接超时：10秒
pub const WS_CONNECT_TIMEOUT_SECS: u64 = 10;

// 重连初始退避：1秒
pub const WS_RECONNECT_INITIAL_DELAY_SECS: u64 = 1;

// 重连最大退避：60秒
pub const WS_RECONNECT_MAX_DELAY_SECS: u64 = 60;

// 连接最大存活：23小时（82800秒），到期主动重连
pub const WS_MAX_LIFETIME_SECS: u64 = 82_800;

// 最大重连次数：超过则熔断
pub const WS_MAX_RETRIES: u64 = 100;

// WS管理器配置
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
    #[cfg(test)]
    pub fn with_pong_timeout(mut self, secs: u64) -> Self {
        self.pong_timeout_secs = secs;
        self
    }

    #[cfg(test)]
    pub fn with_max_retries(mut self, n: u64) -> Self {
        self.max_retries = n;
        self
    }

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

// WS消息处理器接口：由各业务 Handler 实现以解析消息、生成订阅等
#[async_trait]
pub trait WsHandler<T: Send + Clone + 'static>: Send + Sync {
    // 返回连接 URL
    fn base_url(&self) -> &str;

    // 是否支持动态订阅/退订命令，默认 false
    fn supports_commands(&self) -> bool {
        false
    }

    // 重连时刷新 URL（如 listenKey 场景需重新获取），默认返回 base_url
    async fn refresh_url(&self) -> Result<String, ExchangeError> {
        Ok(self.base_url().to_string())
    }

    // 消息解析，返回 MessageOutcome
    async fn on_message(&self, text: &str) -> Result<MessageOutcome<T>, ExchangeError>;

    // 连接后发送的初始订阅消息列表
    async fn on_connected(&self, is_reconnect: bool) -> Vec<String>;

    // 断连回调
    async fn on_disconnected(&self);

    // 动态订阅命令转 JSON 文本，不支持时返回 None
    async fn on_command(&self, _cmd: WsCommand) -> Option<String> {
        None
    }
}

// WS生命周期管理：连接/重连/指数退避/熔断
pub struct WsManager<T: Send + Clone + 'static> {
    config: WsManagerConfig,
    handler: Arc<dyn WsHandler<T>>,
    running: Arc<AtomicBool>,
    retry_count: Arc<AtomicU64>,
    shutdown_tx: Mutex<Option<mpsc::Sender<()>>>,
    command_tx: Mutex<Option<mpsc::UnboundedSender<WsCommand>>>,
}

impl<T: Send + Clone + 'static> WsManager<T> {
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

    pub fn running_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.running)
    }

    pub async fn start(&self, event_tx: mpsc::Sender<WsManagerEvent<T>>) {
        // 防止重复启动
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        *self.shutdown_tx.lock().await = Some(shutdown_tx);

        // 仅在 handler 支持动态订阅时建立命令通道
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

                // 重连前刷新 URL（如 listenKey 场景）
                let ws_url = match handler.refresh_url().await {
                    Ok(url) => url,
                    Err(e) => {
                        tracing::error!(
                            error = %e,
                            "refresh_url failed — will retry after backoff"
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

                // 带超时的连接尝试
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

                        let is_reconnect = !is_first_connect;
                        if event_tx
                            .send(WsManagerEvent::ConnectionChanged {
                                connected: true,
                                is_reconnect,
                            })
                            .await
                            .is_err()
                        {
                            tracing::warn!("Event channel closed on connect, stopping");
                            running.store(false, Ordering::Relaxed);
                            break;
                        }
                        is_first_connect = false;

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
                                tracing::warn!(
                                    "Failed to send on_connected message, reconnecting"
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

                            // 主事件循环：读消息/发PING/处理命令/监听关闭
                            loop {
                                if !running.load(Ordering::Relaxed) {
                                    break;
                                }

                                // 连接达到最大存活时间，主动重连
                                if connect_start.elapsed() > max_lifetime {
                                    tracing::info!(
                                        max_lifetime_secs = config.max_lifetime_secs,
                                        "Max lifetime reached, reconnecting"
                                    );
                                    break;
                                }

                                // PONG 超时：长时间无消息则强制重连
                                if last_msg_time.elapsed()
                                    > Duration::from_secs(config.pong_timeout_secs)
                                {
                                    tracing::warn!(
                                    pong_timeout_secs = config.pong_timeout_secs,
                                    "No message received within pong timeout, forcing reconnect"
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
                                                                    "Event channel closed — stopping WS"
                                                                );
                                                                running.store(false, Ordering::Relaxed);
                                                                break;
                                                            }
                                                        }
                                                    }
                                                    Ok(MessageOutcome::Reconnect) => {
                                                        tracing::info!(
                                                            "Handler requested reconnect"
                                                        );
                                                        break;
                                                    }
                                                    Err(e) => {
                                                        tracing::warn!(
                                                            error = %e,
                                                            msg_preview = %&text[..text.len().min(200)],
                                                            "on_message error — skipping, WS continues"
                                                        );
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
                                            Some(Ok(tungstenite::Message::Binary(_))) => {}
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
                                        let ping = tungstenite::Message::Ping(vec![].into());
                                        if write.send(ping).await.is_err() {
                                            tracing::warn!("Ping failed, reconnecting");
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
                                                        "Command send failed, reconnecting"
                                                    );
                                                    break;
                                                }
                                            }
                                        } else {
                                            break;
                                        }
                                    }
                                    _ = shutdown_rx.recv() => {
                                        tracing::info!("Shutdown signal received, closing");
                                        let _ = write.send(tungstenite::Message::Close(None)).await;
                                        running.store(false, Ordering::Relaxed);
                                        handler.on_disconnected().await;
                                        return;
                                    }
                                }
                            }
                        }

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
                                "Event channel closed on disconnect, stopping"
                            );
                            running.store(false, Ordering::Relaxed);
                            break;
                        }
                    }
                    Ok(Err(e)) => {
                        tracing::error!(error = %e, "Connection failed");
                    }
                    Err(_) => {
                        tracing::error!(
                            timeout_secs = config.connect_timeout_secs,
                            "Connection timeout"
                        );
                    }
                }

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

    pub async fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(tx) = self.shutdown_tx.lock().await.take() {
            let _ = tx.send(()).await;
        }
    }

    // 发送动态订阅/退订命令
    pub async fn send_command(&self, cmd: WsCommand) {
        if let Some(tx) = self.command_tx.lock().await.as_ref() {
            let _ = tx.send(cmd);
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn retry_count(&self) -> u64 {
        self.retry_count.load(Ordering::Relaxed)
    }

    // 指数退避+抖动：超过最大重连次数则熔断；返回 false 表示停止重连
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

        // 熔断检查：超过最大重连次数则触发熔断并停止
        if config.max_retries > 0 {
            let retries = retry_count.fetch_add(1, Ordering::Relaxed) + 1;
            if retries >= config.max_retries {
                tracing::error!(
                    retries = retries,
                    max_retries = config.max_retries,
                    "Max retries exceeded — circuit breaker tripped"
                );
                let _ = event_tx
                    .send(WsManagerEvent::CircuitBreakerTripped {
                        retry_count: retries,
                    })
                    .await;
                return false;
            }
        }

        // 指数退避 + 20% 随机抖动，避免雪崩
        let jitter = rand::random::<f64>() * *reconnect_delay as f64 * 0.2;
        let delay = *reconnect_delay as f64 + jitter;
        tokio::time::sleep(Duration::from_secs_f64(delay)).await;

        // 退避时间翻倍，上限为 reconnect_max_delay_secs
        *reconnect_delay = (*reconnect_delay * 2).min(config.reconnect_max_delay_secs);

        running.load(Ordering::Relaxed)
    }
}
