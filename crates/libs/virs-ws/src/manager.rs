use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{connect_async, tungstenite};
use virs_error::VirsError;
use virs_task::{spawn, CancellationToken, TaskHandle};

pub const WS_PING_INTERVAL_SECS: u64 = 30;
pub const WS_PONG_TIMEOUT_SECS: u64 = 90;
pub const WS_CONNECT_TIMEOUT_SECS: u64 = 10;
pub const WS_RECONNECT_INITIAL_DELAY_SECS: u64 = 1;
pub const WS_RECONNECT_MAX_DELAY_SECS: u64 = 60;
pub const WS_MAX_LIFETIME_SECS: u64 = 82_800;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionReason {
    Connected,
    Reconnected,
    DisconnectedReconnecting,
    Stopped,
}

#[derive(Debug, Clone)]
pub enum WsManagerEvent<T: Send + Clone + 'static> {
    Message(T),

    ConnectionChanged {
        connected: bool,
        reason: ConnectionReason,
    },

    CircuitBreakerTripped { retry_count: u64 },
}

#[derive(Debug, Clone)]
pub enum WsCommand {
    Subscribe(String),
    Unsubscribe(String),
}

#[derive(Debug, Clone)]
pub enum MessageOutcome<T: Send + Clone + 'static> {
    Continue(Vec<T>),
    Reconnect,
}

enum BackoffOutcome {
    Proceed,
    CircuitBroken,
    Shutdown,
}

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

    pub async fn start(
        &self,
        config: WsManagerConfig,
        event_tx: mpsc::Sender<WsManagerEvent<T>>,
    ) {
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

        let handle = spawn("ws_main", move |cancel| async move {
            let mut reconnect_delay = config.reconnect_initial_delay_secs;
            let mut is_first_connect = true;

            while !cancel.is_cancelled() {
                let connect_start = tokio::time::Instant::now();

                let ws_url = match handler.refresh_url().await {
                    Ok(url) => url,
                    Err(e) => {
                        tracing::error!(error = %e, "refresh_url failed");
                        match backoff_with_cancel(
                            &cancel,
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
                            let ping_start = tokio::time::Instant::now()
                                + Duration::from_secs(config.ping_interval_secs);
                            let mut ping_tick = tokio::time::interval_at(
                                ping_start,
                                Duration::from_secs(config.ping_interval_secs),
                            );
                            let max_lifetime = Duration::from_secs(config.max_lifetime_secs);
                            let mut last_msg_time = tokio::time::Instant::now();

                            loop {
                                if cancel.is_cancelled() {
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
                                    _ = cancel.cancelled() => {
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
                    &cancel,
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

    pub async fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        let handle = self.task.lock().unwrap().take();
        if let Some(h) = handle {
            h.cancel();
            h.join_with_timeout(Duration::from_secs(5)).await;
        }
    }

    pub async fn send_command(&self, cmd: WsCommand) {
        if let Some(tx) = self.command_tx.lock().await.as_ref() {
            let _ = tx.send(cmd);
        }
    }
}

async fn backoff_with_cancel<T: Send + Clone + 'static>(
    cancel: &CancellationToken,
    retry_count: &AtomicU64,
    config: &WsManagerConfig,
    reconnect_delay: &mut u64,
    event_tx: &mpsc::Sender<WsManagerEvent<T>>,
) -> BackoffOutcome {
    if cancel.is_cancelled() {
        return BackoffOutcome::Shutdown;
    }

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

    let jitter = rand::random::<f64>() * *reconnect_delay as f64 * 0.2;
    let delay = *reconnect_delay as f64 + jitter;

    tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs_f64(delay)) => {
            *reconnect_delay = (*reconnect_delay * 2).min(config.reconnect_max_delay_secs);
            BackoffOutcome::Proceed
        }
        _ = cancel.cancelled() => {
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
