//! EngineManager — lazy initialization of trading engines.
//!
//! Engines (Position, Grid, Auto) are NOT started at application boot.
//! Instead, they are started when the first bot is created after the wizard,
//! using the exchange credentials provided by the user.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock, Mutex as StdMutex};

use async_trait::async_trait;
use tokio::sync::{broadcast, mpsc, Mutex};
use tracing::info;

use virs_api::EngineManager;
use virs_bot::auto::types::AutoCommand;
use virs_bot::grid::types::GridCommand;
use virs_exchange::{CcxtExchangeAdapter, Exchanges, PaperExchangeAdapter};
use virs_error::VirsResult;
use virs_market::{KlineEngine, OrderBookEngine};
use virs_position::{Persistence as PePersistence, PositionEngine};
use virs_types::bot::{OrderEvent, PriceProvider};
use virs_types::enums::MarketType;
use virs_types::exchange_pe::ExchangePe;
use virs_types::position::{EngineCommand, EngineEvent};

use crate::adapters::*;

/// Inner state — populated on first `ensure_started` call.
struct EngineState {
    paper_mode: bool,
    grid_cmd_tx: StdMutex<Option<mpsc::Sender<GridCommand>>>,
    auto_cmd_tx: StdMutex<Option<mpsc::Sender<AutoCommand>>>,
    /// Position Engine 事件广播器（用于 /ws/position 推送）
    /// shutdown 时 take() 丢弃，使 /ws/position handler 收到 broadcast Closed
    pe_event_tx: StdMutex<Option<broadcast::Sender<EngineEvent>>>,
    /// Position Engine 共享引用（用于查询当前仓位快照）
    /// shutdown 时 take() 丢弃，释放 cmd_tx clone + Arc<EngineInner> 引用
    position_engine: StdMutex<Option<PositionEngine>>,
    /// Symbols that need price ticks in paper mode (exchange, symbol)
    paper_symbols: Arc<Mutex<Vec<(String, String)>>>,
    /// JoinHandle for paper mode price tick task
    paper_tick_handle: StdMutex<Option<tokio::task::JoinHandle<()>>>,
    /// JoinHandle for PositionEngine run loop
    pe_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// JoinHandle for GridEngine run loop
    grid_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// JoinHandle for AutoEngine run loop
    auto_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

/// Application-level engine manager.
pub struct AppEngineManager {
    db_pool: sqlx::PgPool,
    exchange_registry: Arc<Exchanges>,
    kline_engine: Arc<KlineEngine>,
    orderbook_engine: Arc<OrderBookEngine>,
    encryption_key: String,
    llm_key: String,
    #[allow(dead_code)]
    proxy: Option<String>,

    started: AtomicBool,
    /// Init lock — ensures only one caller runs the init logic.
    init_lock: Mutex<()>,
    /// Cached state — set once after init, readable without async.
    state: OnceLock<EngineState>,
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
    ) -> Self {
        Self {
            db_pool,
            exchange_registry,
            kline_engine,
            orderbook_engine,
            encryption_key,
            llm_key,
            proxy,
            started: AtomicBool::new(false),
            init_lock: Mutex::new(()),
            state: OnceLock::new(),
        }
    }
}

#[async_trait]
impl EngineManager for AppEngineManager {
    async fn ensure_started(&self, paper_mode: bool) -> VirsResult<()> {
        // Fast path — already started
        if self.started.load(Ordering::SeqCst) {
            return Ok(());
        }

        let _guard = self.init_lock.lock().await;

        // Double-check after acquiring lock
        if self.state.get().is_some() {
            return Ok(());
        }

        info!("Starting trading engines (paper_mode={})...", paper_mode);

        // ── Position Engine ──
        let pe_exchange: Box<dyn ExchangePe> = if paper_mode {
            let initial_balance = {
                let temp_adapter = CcxtExchangeAdapter::new(self.exchange_registry.clone());
                match temp_adapter.get_balance().await {
                    Ok(b) => {
                        b.total
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Paper mode: failed to fetch real balance, using 0");
                        0.0
                    }
                }
            };
            Box::new(
                PaperExchangeAdapter::new("paper", MarketType::Perpetual, initial_balance)
                    .with_exchange_registry(self.exchange_registry.clone()),
            )
        } else {
            Box::new(CcxtExchangeAdapter::new(self.exchange_registry.clone()))
        };

        let pe_persistence = Box::new(PePersistence::new(self.db_pool.clone()));
        let pe_config = virs_types::position::EngineConfig::default();

        let mut position_engine = PositionEngine::new(pe_config, pe_exchange, pe_persistence);
        let pe_cmd_tx = position_engine.command_sender();
        let pe_event_sender = position_engine.event_sender();
        let grid_pe_event_rx = position_engine.subscribe_events();
        let auto_pe_event_rx = position_engine.subscribe_events();
        let pe_exchange_ref = position_engine.exchange();
        // 保存 clone 用于后续查询当前仓位快照（/ws/position subscribe 时推送）
        let position_engine_clone = position_engine.clone();

        let pe_handle = tokio::spawn(async move {
            if let Err(e) = position_engine.run().await {
                tracing::error!(error = %e, "Position Engine run failed");
            }
        });
        info!("Position Engine started (paper={})", paper_mode);

        // ── Grid Engine ──
        let (grid_event_tx, _grid_event_rx) = tokio::sync::broadcast::channel(256);

        let grid_store = Arc::new(PgGridStore::new(self.db_pool.clone()));
        let grid_price_provider = Arc::new(
            ExchangePriceProvider::new(self.exchange_registry.clone())
                .with_kline_engine(self.kline_engine.clone()),
        );
        let grid_market_data_provider = Arc::new(
            ExchangeMarketDataProvider::new(self.exchange_registry.clone())
                .with_kline_engine(self.kline_engine.clone())
                .with_pe_exchange(pe_exchange_ref.clone()),
        );
        let grid_order_executor = Arc::new(PeOrderExecutor::new(
            pe_cmd_tx.clone(),
            grid_event_tx.clone(),
            grid_pe_event_rx,
            position_engine_clone.clone(),
        ));
        let grid_credential_store: Arc<dyn virs_types::bot::CredentialStore> =
            Arc::new(PgCredentialStore::new(
                self.db_pool.clone(),
                virs_utils::crypto::derive_key(&self.llm_key),
            ));
        let grid_llm_resolver: Arc<dyn virs_types::bot::LlmProviderResolver> =
            Arc::new(DefaultLlmResolver::new());
        let grid_ai_service = Arc::new(virs_bot::grid::ai::GridAiService::new(
            grid_llm_resolver,
            grid_credential_store,
        ));

        let (mut grid_engine, grid_cmd_tx, _grid_event_broadcast) = virs_bot::grid::GridEngine::new(
            grid_store,
            grid_ai_service,
            grid_price_provider.clone(),
            grid_order_executor,
            grid_market_data_provider,
            grid_event_tx.clone(),
        );

        // Paper mode price tick
        let paper_symbols: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
        let paper_tick_handle: Option<tokio::task::JoinHandle<()>> = if paper_mode {
            let price_provider_for_paper: Arc<dyn PriceProvider> = grid_price_provider.clone();
            let kline_engine_for_paper = self.kline_engine.clone();
            let pe_cmd_tx_for_tick = pe_cmd_tx.clone();
            let paper_symbols_for_tick = paper_symbols.clone();
            Some(tokio::spawn(async move {
                let mut tick = tokio::time::interval(std::time::Duration::from_secs(3));
                loop {
                    tick.tick().await;
                    let kline_symbols = kline_engine_for_paper.subscribed_symbols();
                    let symbols: Vec<(String, String)> = if kline_symbols.is_empty() {
                        paper_symbols_for_tick.lock().await.clone()
                    } else {
                        kline_symbols.into_iter().map(|(e, s, _)| (e, s)).collect()
                    };
                    for (exchange, symbol) in symbols {
                        if let Some(price) = price_provider_for_paper
                            .get_price(&exchange, &symbol, "perpetual")
                            .await
                        {
                            if pe_cmd_tx_for_tick
                                .send(EngineCommand::PriceTick {
                                    symbol: symbol.clone(),
                                    price,
                                })
                                .await
                                .is_err()
                            {
                                return;
                            }
                        }
                    }
                }
            }))
        } else {
            None
        };

        let grid_handle = tokio::spawn(async move {
            grid_engine.run().await;
        });
        info!("Grid engine started");

        // ── Auto Trade Engine ──
        let auto_store = Arc::new(PgAutoStore::new(self.db_pool.clone()));
        let auto_price_provider = Arc::new(
            AutoExchangePriceProvider::new(self.exchange_registry.clone())
                .with_kline_engine(self.kline_engine.clone()),
        );
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
        ));
        let auto_credential_store: Arc<dyn virs_types::bot::CredentialStore> =
            Arc::new(PgCredentialStore::new(
                self.db_pool.clone(),
                virs_utils::crypto::derive_key(&self.llm_key),
            ));
        let auto_llm_resolver: Arc<dyn virs_types::bot::LlmProviderResolver> =
            Arc::new(DefaultLlmResolver::new());
        let auto_ai_service = Arc::new(virs_bot::auto::ai::AutoAiService::new(
            auto_llm_resolver,
            auto_credential_store,
        ));

        let (mut auto_engine, auto_cmd_tx) = virs_bot::auto::AutoEngine::new(
            auto_store,
            auto_ai_service,
            auto_price_provider,
            auto_order_executor,
            auto_market_data_provider,
            auto_order_event_tx.clone(),
            pe_event_sender.clone(),
        );

        let auto_handle = tokio::spawn(async move {
            auto_engine.run().await;
        });
        info!("Auto trade engine started");

        // Register existing running auto bot symbols for paper mode price ticks
        if paper_mode {
            let db = self.db_pool.clone();
            let ps = paper_symbols.clone();
            tokio::spawn(async move {
                let bots: Vec<(String, String)> = sqlx::query_as(
                    r#"SELECT DISTINCT exchange, symbol FROM qd_auto_bots WHERE status = 'running'"#
                )
                .fetch_all(&db)
                .await
                .unwrap_or_default();
                let mut symbols = ps.lock().await;
                for (exchange, symbol) in bots {
                    if !symbols.contains(&(exchange.clone(), symbol.clone())) {
                        symbols.push((exchange, symbol));
                    }
                }
            });
        }

        // Store state (OnceLock — set once, then readable synchronously)
        let _ = self.state.set(EngineState {
            paper_mode,
            grid_cmd_tx: StdMutex::new(Some(grid_cmd_tx)),
            auto_cmd_tx: StdMutex::new(Some(auto_cmd_tx)),
            pe_event_tx: StdMutex::new(Some(pe_event_sender)),
            position_engine: StdMutex::new(Some(position_engine_clone)),
            paper_symbols: paper_symbols.clone(),
            paper_tick_handle: StdMutex::new(paper_tick_handle),
            pe_handle: Mutex::new(Some(pe_handle)),
            grid_handle: Mutex::new(Some(grid_handle)),
            auto_handle: Mutex::new(Some(auto_handle)),
        });
        self.started.store(true, Ordering::SeqCst);

        info!("All trading engines started successfully");
        Ok(())
    }

    fn grid_cmd_tx(&self) -> Option<mpsc::Sender<GridCommand>> {
        self.state.get().and_then(|s| s.grid_cmd_tx.lock().unwrap().clone())
    }

    fn auto_cmd_tx(&self) -> Option<mpsc::Sender<AutoCommand>> {
        self.state.get().and_then(|s| s.auto_cmd_tx.lock().unwrap().clone())
    }

    fn paper_mode(&self) -> bool {
        self.state.get().map(|s| s.paper_mode).unwrap_or(false)
    }

    async fn register_paper_symbol(&self, exchange: String, symbol: String) {
        if let Some(state) = self.state.get() {
            let mut symbols = state.paper_symbols.lock().await;
            if !symbols.contains(&(exchange.clone(), symbol.clone())) {
                symbols.push((exchange, symbol));
            }
        }
    }

    fn pe_event_subscribe(&self) -> Option<broadcast::Receiver<EngineEvent>> {
        self.state
            .get()
            .and_then(|s| s.pe_event_tx.lock().unwrap().as_ref().map(|tx| tx.subscribe()))
    }

    fn get_positions_by_symbol(&self, symbol: &str) -> Vec<virs_types::position::Position> {
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
        // Already started — nothing to do
        if self.started.load(Ordering::SeqCst) {
            return Ok(());
        }

        // Check if any bots exist in DB
        let has_bots: bool = {
            let grid_count: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM qd_grid_bots"#)
        .fetch_one(&self.db_pool)
        .await?;

    let auto_count: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM qd_auto_bots"#)
        .fetch_one(&self.db_pool)
        .await?;

            grid_count + auto_count > 0
        };

        if !has_bots {
            return Ok(());
        }

        // 1. Restore Exchanges from DB credentials
        let rows: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
            r#"SELECT exchange, encrypted_api_key, encrypted_api_secret, encrypted_passphrase
               FROM qd_exchange_credentials"#,
        )
        .fetch_all(&self.db_pool)
        .await
        .unwrap_or_default();

        for (exchange, enc_key, enc_secret, enc_passphrase) in &rows {
            let api_key = match virs_utils::crypto::decrypt_with_key(enc_key, &self.encryption_key) {
                Ok(k) => k,
                Err(e) => {
                    tracing::warn!(exchange, "Failed to decrypt API key: {}", e);
                    continue;
                }
            };
            let api_secret = match virs_utils::crypto::decrypt_with_key(enc_secret, &self.encryption_key) {
                Ok(k) => k,
                Err(e) => {
                    tracing::warn!(exchange, "Failed to decrypt API secret: {}", e);
                    continue;
                }
            };
            let passphrase = enc_passphrase
                .as_ref()
                .and_then(|p| virs_utils::crypto::decrypt_with_key(p, &self.encryption_key).ok());

            // Try both market types (perpetual first, then spot)
            for mt_str in &["perpetual", "spot"] {
                let exchange_key = format!("{}:{}", exchange, mt_str);
                if self.exchange_registry.get(&exchange_key).is_some() {
                    continue; // Already registered
                }

                let ccxt_mt = match *mt_str {
                    "spot" => virs_ccxt::MarketType::Spot,
                    _ => virs_ccxt::MarketType::Perpetual,
                };

                if let Ok(ccxt_ex) = virs_ccxt::create_exchange(
                    exchange,
                    &api_key,
                    &api_secret,
                    passphrase.as_deref(),
                    None,
                    &ccxt_mt,
                ) {
                    let app_mt = match *mt_str {
                        "spot" => MarketType::Spot,
                        _ => MarketType::Perpetual,
                    };
                    let adapter = virs_exchange::CcxtAdapter::new(ccxt_ex, app_mt);
                    self.exchange_registry.register(Box::new(adapter));
                }
            }
        }

        // 2. Restore Kline subscriptions for running bot symbols
        let bot_symbols: Vec<(String, String, String)> = sqlx::query_as(
            r#"
            SELECT exchange, symbol, market_type FROM qd_auto_bots WHERE status = 'running'
            UNION
            SELECT exchange, symbol, market_type FROM qd_grid_bots WHERE status = 'running'
            "#,
        )
        .fetch_all(&self.db_pool)
        .await
        .unwrap_or_default();

        for (exchange, symbol, market_type) in &bot_symbols {
            let mt = match market_type.as_str() {
                "spot" => virs_models::MarketType::Spot,
                _ => virs_models::MarketType::Perpetual,
            };
            if let Err(e) = self.kline_engine.subscribe(exchange, symbol, mt).await {
                tracing::warn!(
                    exchange,
                    symbol,
                    "Failed to restore kline subscription: {}",
                    e
                );
            } else {
                info!(exchange, symbol, market_type, "Restored kline subscription");
            }
            if let Err(e) = self.orderbook_engine.subscribe(exchange, symbol, mt).await {
                tracing::warn!(
                    exchange,
                    symbol,
                    "Failed to restore orderbook subscription: {}",
                    e
                );
            } else {
                info!(
                    exchange,
                    symbol, market_type, "Restored orderbook subscription"
                );
            }
        }

        // 3. Determine paper mode from DB (set by the wizard when creating bots)
        let paper_mode: bool = sqlx::query_scalar(
            r#"SELECT paper_mode FROM qd_auto_bots WHERE status = 'running' LIMIT 1
               UNION ALL
               SELECT paper_mode FROM qd_grid_bots WHERE status = 'running' LIMIT 1"#,
        )
        .fetch_optional(&self.db_pool)
        .await?
        .unwrap_or(true); // Default to safe paper trading

        // 4. Start engines (which will call restore_running_bots internally)
        if let Err(e) = self.ensure_started(paper_mode).await {
            tracing::error!("Failed to restore engines: {}", e);
        }

        Ok(())
    }

    async fn shutdown(&self) {
        if let Some(state) = self.state.get() {
            info!("Shutting down trading engines...");

            // 1. Signal PositionEngine to stop (sets ShuttingDown state)
            //    Take position_engine clone to call stop(), then drop its cmd_tx
            let pe_opt = state.position_engine.lock().unwrap().take();
            if let Some(pe) = &pe_opt {
                pe.stop();
            }

            // 2. Abort paper price tick task (holds pe_cmd_tx clone)
            if let Some(handle) = state.paper_tick_handle.lock().unwrap().take() {
                handle.abort();
            }

            // 3. Drop command senders to trigger grid/auto cmd_loop exit
            //    (recv() returns None when all senders are dropped)
            drop(state.grid_cmd_tx.lock().unwrap().take());
            drop(state.auto_cmd_tx.lock().unwrap().take());

            // 4. Drop pe_event_tx so /ws/position handlers see broadcast Closed
            drop(state.pe_event_tx.lock().unwrap().take());

            // 5. Take JoinHandles and await all three in parallel (with timeout)
            let pe_handle = state.pe_handle.lock().await.take();
            let grid_handle = state.grid_handle.lock().await.take();
            let auto_handle = state.auto_handle.lock().await.take();

            let timeout = std::time::Duration::from_secs(5);
            let _ = tokio::time::timeout(timeout, async {
                let pe_fut = async { if let Some(h) = pe_handle { let _ = h.await; } };
                let grid_fut = async { if let Some(h) = grid_handle { let _ = h.await; } };
                let auto_fut = async { if let Some(h) = auto_handle { let _ = h.await; } };
                tokio::join!(pe_fut, grid_fut, auto_fut);
            }).await;

            // 6. Drop pe_opt — releases the last cmd_tx clone (other than
            //    the one inside the run() task, which is dropped when run() returns)
            drop(pe_opt);

            info!("All trading engines stopped");
        }
    }
}
