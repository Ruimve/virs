use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::{broadcast, mpsc, Mutex};
use tracing::{error, info, warn};
use virs_task::{spawn, Stop, TaskHandle};

use virs_api::EngineManager;
use virs_bot::auto::types::AutoCommand;
use virs_error::VirsResult;
use virs_exchange::{Exchanges, PaperExchangeAdapter};
use virs_market::{KlineEngine, OrderBookEngine};
use virs_position::{Persistence as PePersistence, PositionEngine};
use virs_strategy::prompt::PromptLoader;
use virs_type::bot::OrderEvent;
use virs_type::MarketType;
use virs_type::exchange::ExchangePe;
use virs_type::position::{EngineCommand, EngineEvent};

use crate::adapters::*;

struct EngineState {
    paper_mode: bool,
    auto_cmd_tx: StdMutex<Option<mpsc::Sender<AutoCommand>>>,
    pe_event_tx: StdMutex<Option<broadcast::Sender<EngineEvent>>>,
    position_engine: StdMutex<Option<PositionEngine>>,
    position_engine_task: StdMutex<Option<TaskHandle>>,
    paper_tick_task: StdMutex<Option<TaskHandle>>,
    auto_engine_task: StdMutex<Option<TaskHandle>>,
    order_executor: StdMutex<Option<Arc<PeOrderExecutor>>>,
}

pub struct AppEngineManager {
    db_pool: sqlx::PgPool,
    exchange_registry: Arc<Exchanges>,
    kline_engine: Arc<KlineEngine>,
    orderbook_engine: Arc<OrderBookEngine>,
    encryption_key: String,
    llm_key: String,
    proxy: Option<String>,

    time_config: virs_config::TimeConfig,

    prompt_loader: PromptLoader,

    started: AtomicBool,

    init_lock: Mutex<()>,

    state: OnceLock<EngineState>,

    restore_error: StdMutex<Option<String>>,
}

impl AppEngineManager {
    pub fn new(
        db_pool: sqlx::PgPool,
        exchange_registry: Arc<Exchanges>,
        kline_engine: Arc<KlineEngine>,
        orderbook_engine: Arc<OrderBookEngine>,
        encryption_key: String,
        llm_key: String,
        proxy: Option<String>,
        time_config: virs_config::TimeConfig,
        prompt_loader: PromptLoader,
    ) -> Self {
        Self {
            db_pool,
            exchange_registry,
            kline_engine,
            orderbook_engine,
            encryption_key,
            llm_key,
            proxy,
            time_config,
            prompt_loader,
            started: AtomicBool::new(false),
            init_lock: Mutex::new(()),
            state: OnceLock::new(),
            restore_error: StdMutex::new(None),
        }
    }

    async fn restore_inner(&self) -> VirsResult<()> {
        let has_bots: bool = {
            let auto_count: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM qd_auto_bots"#)
                .fetch_one(&self.db_pool)
                .await?;
            auto_count > 0
        };

        if !has_bots {
            return Ok(());
        }

        let rows: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
            r#"SELECT exchange, encrypted_api_key, encrypted_api_secret, encrypted_passphrase
               FROM qd_exchange_credentials"#,
        )
        .fetch_all(&self.db_pool)
        .await?;

        for (exchange, enc_key, enc_secret, enc_passphrase) in &rows {
            let api_key = virs_utils::crypto::decrypt_with_key(enc_key, &self.encryption_key)
                .map_err(|e| {
                    virs_error::VirsError::config(format!(
                        "Failed to decrypt API key for exchange '{}': {}",
                        exchange, e
                    ))
                })?;
            let api_secret = virs_utils::crypto::decrypt_with_key(enc_secret, &self.encryption_key)
                .map_err(|e| {
                    virs_error::VirsError::config(format!(
                        "Failed to decrypt API secret for exchange '{}': {}",
                        exchange, e
                    ))
                })?;
            let passphrase = match enc_passphrase.as_ref() {
                Some(p) => Some(
                    virs_utils::crypto::decrypt_with_key(p, &self.encryption_key).map_err(|e| {
                        virs_error::VirsError::config(format!(
                            "Failed to decrypt passphrase for exchange '{}': {}",
                            exchange, e
                        ))
                    })?,
                ),
                None => None,
            };

            for mt_str in &["perpetual"] {
                let exchange_key = format!("{}:{}", exchange, mt_str);
                if self.exchange_registry.get(&exchange_key).is_some() {
                    continue;
                }

                let exchange_instance = virs_ccxt::create_exchange(
                    exchange,
                    &api_key,
                    &api_secret,
                    passphrase.as_deref(),
                    self.proxy.as_deref(),
                    std::time::Duration::from_secs(self.time_config.http_timeout_secs),
                    std::time::Duration::from_secs(self.time_config.http.http_connect_timeout_secs),
                    self.time_config.http.http_pool_max_idle_per_host,
                    self.time_config.listenkey.listenkey_keepalive_futures_secs,
                )
                .await
                .map_err(|e| {
                    virs_error::VirsError::config(format!(
                        "Failed to create exchange '{}' ({}): {}",
                        exchange, mt_str, e
                    ))
                })?;

                self.exchange_registry.register(exchange_instance);
                info!(exchange = %exchange, "Restored exchange credential");
            }
        }

        let bot_symbols: Vec<(String, String)> = sqlx::query_as(
            r#"SELECT exchange, symbol FROM qd_auto_bots WHERE status = 'running'"#,
        )
        .fetch_all(&self.db_pool)
        .await?;

        for (exchange, symbol) in &bot_symbols {
            let mt = MarketType::Perpetual;
            self.kline_engine
                .subscribe(exchange, symbol, mt)
                .await
                .map_err(|e| {
                    virs_error::VirsError::config(format!(
                        "Failed to restore kline subscription for {} {}: {}",
                        exchange, symbol, e
                    ))
                })?;
            info!(exchange = %exchange, symbol = %symbol, "Restored kline subscription");

            self.orderbook_engine
                .subscribe(exchange, symbol, mt)
                .await
                .map_err(|e| {
                    virs_error::VirsError::config(format!(
                        "Failed to restore orderbook subscription for {} {}: {}",
                        exchange, symbol, e
                    ))
                })?;
            info!(exchange = %exchange, symbol = %symbol, "Restored orderbook subscription");
        }

        let paper_modes: Vec<bool> = sqlx::query_scalar(
            r#"SELECT DISTINCT paper_mode FROM qd_auto_bots WHERE status = 'running'"#,
        )
        .fetch_all(&self.db_pool)
        .await?;

        if paper_modes.is_empty() {
            info!("No running bots found — engines will start on first bot creation");
            return Ok(());
        }

        if paper_modes.len() > 1 {
            return Err(virs_error::VirsError::config(
                "Inconsistent paper_mode values among running bots — \
                 cannot determine engine mode. All running bots must share \
                 the same paper_mode.",
            ));
        }

        let paper_mode = paper_modes[0];

        self.ensure_started(paper_mode).await?;

        Ok(())
    }

    async fn mark_running_bots_as_error(&self) -> VirsResult<()> {
        sqlx::query(r#"UPDATE qd_auto_bots SET status = 'error', stopped_at = NOW() WHERE status = 'running'"#)
            .execute(&self.db_pool)
            .await?;
        error!("Marked all running bots as 'error' due to restore failure");
        Ok(())
    }
}

#[async_trait]
impl EngineManager for AppEngineManager {
    async fn ensure_started(&self, paper_mode: bool) -> VirsResult<()> {
        if self.started.load(Ordering::SeqCst) {
            return Ok(());
        }

        let _guard = self.init_lock.lock().await;

        if self.state.get().is_some() {
            return Ok(());
        }

        info!(paper_mode, "Starting trading engines");

        let pe_exchange: Arc<dyn ExchangePe> = if paper_mode {
            let initial_balance = match self.exchange_registry.get_perpetual() {
                Some(ex) => match ex.get_balance().await {
                    Ok(b) => b.total,
                    Err(e) => {
                        warn!(error = %e, mode = "paper", "Failed to fetch real balance, using 0");
                        0.0
                    }
                },
                None => {
                    warn!(mode = "paper", "No perpetual exchange registered, using 0 balance");
                    0.0
                }
            };
            Arc::new(
                PaperExchangeAdapter::new("paper", MarketType::Perpetual, initial_balance)
                    .with_exchange_registry(self.exchange_registry.clone()),
            )
        } else {
            self.exchange_registry.get_perpetual().ok_or_else(|| {
                virs_error::VirsError::config(
                    "No perpetual exchange registered; please save API credentials first.",
                )
            })?
        };

        let pe_persistence = Box::new(PePersistence::new(self.db_pool.clone()));

        let mut position_engine = PositionEngine::new(
            pe_exchange,
            pe_persistence,
            self.time_config.retry.persist_max_retries,
            self.time_config.retry.persist_retry_base_ms,
        );
        let pe_cmd_tx = position_engine.command_sender();
        let pe_event_sender = position_engine.event_sender();
        let auto_pe_event_rx = position_engine.subscribe_events();
        let pe_exchange_ref = position_engine.exchange();

        let position_engine_clone = position_engine.clone();

        let pe_task = spawn("position_engine", move |_| async move {
            if let Err(e) = position_engine.run().await {
                error!(error = %e, "Position Engine run failed");
            }
        });
        info!(paper_mode, "Position Engine started");

        let mut paper_tick_task_opt: Option<TaskHandle> = None;

        let prompt_loader = self.prompt_loader.clone();

        if paper_mode {
            let paper_bots: Vec<(String, String)> = sqlx::query_as(
                r#"SELECT DISTINCT exchange, symbol FROM qd_auto_bots WHERE status = 'running'"#,
            )
            .fetch_all(&self.db_pool)
            .await?;
            for (exchange, symbol) in &paper_bots {
                info!(exchange = %exchange, symbol = %symbol, "Paper mode: kline subscription already restored");
            }
        }

        if paper_mode {
            let kline_engine_for_paper = self.kline_engine.clone();
            let pe_cmd_tx_for_tick = pe_cmd_tx.clone();
            let paper_task = spawn("paper_tick", move |stop: Stop| async move {
                let mut kline_rx = kline_engine_for_paper.subscribe_events();
                info!("Paper mode kline event bridge started");
                loop {
                    tokio::select! {
                        _ = stop.cancelled() => {
                            info!("Paper tick bridge cancelled, stopping");
                            return;
                        }
                        result = kline_rx.recv() => {
                            match result {
                                Ok(event) => {
                                    if event.candle.close > 0.0 {
                                        if pe_cmd_tx_for_tick
                                            .send(EngineCommand::PriceTick {
                                                symbol: event.symbol,
                                                price: event.candle.close,
                                            })
                                            .await
                                            .is_err()
                                        {
                                            info!("PE command channel closed, stopping paper kline bridge");
                                            return;
                                        }
                                    }
                                }
                                Err(broadcast::error::RecvError::Lagged(n)) => {
                                    warn!(lagged = n, "KlineEvent lagged in paper tick bridge");
                                }
                                Err(broadcast::error::RecvError::Closed) => {
                                    info!("KlineEvent channel closed, stopping paper kline bridge");
                                    break;
                                }
                            }
                        }
                    }
                }
            });
            paper_tick_task_opt = Some(paper_task);
        }

        let auto_store = Arc::new(PgAutoStore::new(self.db_pool.clone()));
        let auto_market_data_provider = Arc::new(
            AutoExchangeMarketDataProvider::new(self.exchange_registry.clone())
                .with_kline_engine(self.kline_engine.clone())
                .with_pe_exchange(pe_exchange_ref.clone()),
        );
        let (auto_order_event_tx, _) = tokio::sync::broadcast::channel::<OrderEvent>(256);
        let auto_order_executor = Arc::new(PeOrderExecutor::new(
            pe_cmd_tx.clone(),
            auto_order_event_tx.clone(),
            auto_pe_event_rx,
            position_engine_clone.clone(),
        )
        .await);
        let order_executor_for_state = Arc::clone(&auto_order_executor);
        let auto_credential_store: Arc<dyn virs_type::bot::CredentialStore> =
            Arc::new(PgCredentialStore::new(
                self.db_pool.clone(),
                virs_utils::crypto::derive_key(&self.llm_key),
            ));
        let auto_llm_resolver: Arc<dyn virs_type::bot::LlmProviderResolver> =
            Arc::new(DefaultLlmResolver::new());
        let auto_ai_service = Arc::new(virs_bot::auto::ai::AutoAiService::new(
            auto_llm_resolver,
            auto_credential_store,
            std::time::Duration::from_secs(self.time_config.llm_timeout_secs),
        ));

        let (mut auto_engine, auto_cmd_tx) = virs_bot::auto::AutoEngine::new(
            auto_store,
            auto_ai_service,
            self.kline_engine.clone(),
            auto_order_executor,
            auto_market_data_provider,
            auto_order_event_tx.clone(),
            pe_event_sender.clone(),
            self.time_config.clone(),
            prompt_loader.clone(),
        );

        let auto_task = spawn("auto_engine", move |stop: Stop| async move {
            auto_engine.run(stop).await;
        });
        info!("Auto trade engine started");

        let _ = self.state.set(EngineState {
            paper_mode,
            auto_cmd_tx: StdMutex::new(Some(auto_cmd_tx)),
            pe_event_tx: StdMutex::new(Some(pe_event_sender)),
            position_engine: StdMutex::new(Some(position_engine_clone)),
            position_engine_task: StdMutex::new(Some(pe_task)),
            paper_tick_task: StdMutex::new(paper_tick_task_opt),
            auto_engine_task: StdMutex::new(Some(auto_task)),
            order_executor: StdMutex::new(Some(order_executor_for_state)),
        });
        self.started.store(true, Ordering::SeqCst);

        info!("All trading engines started successfully");
        Ok(())
    }

    fn auto_cmd_tx(&self) -> Option<mpsc::Sender<AutoCommand>> {
        self.state
            .get()
            .and_then(|s| s.auto_cmd_tx.lock().unwrap().clone())
    }

    fn paper_mode(&self) -> Option<bool> {
        self.state.get().map(|s| s.paper_mode)
    }

    fn restore_error(&self) -> Option<String> {
        self.restore_error.lock().unwrap().clone()
    }

    fn pe_event_subscribe(&self) -> Option<broadcast::Receiver<EngineEvent>> {
        self.state.get().and_then(|s| {
            s.pe_event_tx
                .lock()
                .unwrap()
                .as_ref()
                .map(|tx| tx.subscribe())
        })
    }

    fn get_positions_by_symbol(&self, symbol: &str) -> Vec<virs_type::position::Position> {
        match self.state.get() {
            Some(s) => {
                let guard = s.position_engine.lock().unwrap();
                match guard.as_ref() {
                    Some(pe) => pe
                        .get_all_positions()
                        .into_iter()
                        .filter(|p| p.symbol == symbol)
                        .collect(),
                    None => Vec::new(),
                }
            }
            None => Vec::new(),
        }
    }

    async fn restore_if_needed(&self) -> VirsResult<()> {
        if self.started.load(Ordering::SeqCst) {
            return Ok(());
        }

        if let Err(e) = self.restore_inner().await {
            error!(error = %e, "Service restore failed");

            if let Err(db_err) = self.mark_running_bots_as_error().await {
                error!(error = %db_err, "Failed to mark bots as error during restore failure");
            }

            *self.restore_error.lock().unwrap() = Some(e.to_string());

            return Ok(());
        }

        *self.restore_error.lock().unwrap() = None;
        Ok(())
    }

    async fn shutdown(&self) {
        if let Some(state) = self.state.get() {
            info!("Shutting down trading engines...");

            let pe_opt = state.position_engine.lock().unwrap().take();
            if let Some(pe) = &pe_opt {
                pe.stop();
            }

            drop(state.auto_cmd_tx.lock().unwrap().take());
            drop(state.pe_event_tx.lock().unwrap().take());

            let oe_opt = state.order_executor.lock().unwrap().take();
            if let Some(oe) = oe_opt {
                oe.stop().await;
            }

            let pe_task = state.position_engine_task.lock().unwrap().take();
            let paper_task = state.paper_tick_task.lock().unwrap().take();
            let auto_task = state.auto_engine_task.lock().unwrap().take();

            if let Some(h) = &pe_task { h.cancel(); }
            if let Some(h) = &paper_task { h.cancel(); }
            if let Some(h) = &auto_task { h.cancel(); }

            let mut join_set = tokio::task::JoinSet::new();
            if let Some(h) = pe_task {
                join_set.spawn(h.join());
            }
            if let Some(h) = paper_task {
                join_set.spawn(h.join());
            }
            if let Some(h) = auto_task {
                join_set.spawn(h.join());
            }
            while join_set.join_next().await.is_some() {}

            drop(pe_opt);

            info!("All trading engines stopped");
        }
    }
}
