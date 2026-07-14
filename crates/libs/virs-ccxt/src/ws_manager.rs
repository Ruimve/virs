use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{connect_async, tungstenite};
use virs_error::ExchangeError;


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


#[derive(Debug, Clone)]
pub enum WsManagerEvent<T: Send + Clone + 'static> {

    Message(T),


    ConnectionChanged {
        connected: bool,
        is_reconnect: bool,
    },


    CircuitBreakerTripped { retry_count: u64 },
}


pub const WS_PING_INTERVAL_SECS: u64 = 30;


pub const WS_PONG_TIMEOUT_SECS: u64 = 90;


pub const WS_CONNECT_TIMEOUT_SECS: u64 = 10;


pub const WS_RECONNECT_INITIAL_DELAY_SECS: u64 = 1;


pub const WS_RECONNECT_MAX_DELAY_SECS: u64 = 60;


pub const WS_MAX_LIFETIME_SECS: u64 = 82_800;


pub const WS_MAX_RETRIES: u64 = 100;


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


#[async_trait]
pub trait WsHandler<T: Send + Clone + 'static>: Send + Sync {


    fn base_url(&self) -> &str;


    fn supports_commands(&self) -> bool {
        false
    }


    async fn refresh_url(&self) -> Result<String, ExchangeError> {
        Ok(self.base_url().to_string())
    }


    async fn on_message(&self, text: &str) -> Result<MessageOutcome<T>, ExchangeError>;


    async fn on_connected(&self, is_reconnect: bool) -> Vec<String>;


    async fn on_disconnected(&self);


    async fn on_command(&self, _cmd: WsCommand) -> Option<String> {
        None
    }
}


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


                        loop {
                            if !running.load(Ordering::Relaxed) {
                                break;
                            }


                            if connect_start.elapsed() > max_lifetime {
                                tracing::info!(
                                    "[WsManager] Max lifetime ({}s) reached, reconnecting",
                                    config.max_lifetime_secs
                                );
                                break;
                            }


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


        let jitter = rand::random::<f64>() * *reconnect_delay as f64 * 0.2;
        let delay = *reconnect_delay as f64 + jitter;
        tokio::time::sleep(Duration::from_secs_f64(delay)).await;

        *reconnect_delay = (*reconnect_delay * 2).min(config.reconnect_max_delay_secs);

        running.load(Ordering::Relaxed)
    }
}
