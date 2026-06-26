//! EngineManager — lazy initialization of trading engines.
//!
//! Engines (Position, Grid, Auto) are NOT started at application boot.
//! Instead, they are started when the first bot is created after the wizard,
//! using the exchange credentials provided by the user.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use tokio::sync::{broadcast, mpsc, Mutex};
use tracing::info;

use virs_api::EngineManager;
use virs_bot::auto::types::AutoCommand;
use virs_bot::auto::types::AutoEvent;
use virs_bot::grid::types::GridCommand;
use virs_config::AiConfig;
use virs_exchange::{CcxtExchangeAdapter, Exchanges, PaperExchangeAdapter};
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
    grid_cmd_tx: mpsc::Sender<GridCommand>,
    auto_cmd_tx: mpsc::Sender<AutoCommand>,
    /// Position Engine 事件广播器（用于 /ws/position 推送）
    pe_event_tx: broadcast::Sender<EngineEvent>,
    /// Position Engine 共享引用（用于查询当前仓位快照）
    position_engine: PositionEngine,
    /// Symbols that need price ticks in paper mode (exchange, symbol)
    paper_symbols: Arc<Mutex<Vec<(String, String)>>>,
}

/// Application-level engine manager.
pub struct AppEngineManager {
    db_pool: sqlx::PgPool,
    exchange_registry: Arc<Exchanges>,
    kline_engine: Arc<KlineEngine>,
    orderbook_engine: Arc<OrderBookEngine>,
    encryption_key: String,
    ai_config: AiConfig,
    /// Paper trading mode from AppConfig (unified config consumption)
    paper_mode: Option<bool>,
    ws_broadcaster: Arc<virs_api::WsBroadcaster>,
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
        ai_config: AiConfig,
        paper_mode: Option<bool>,
        ws_broadcaster: Arc<virs_api::WsBroadcaster>,
        proxy: Option<String>,
    ) -> Self {
        Self {
            db_pool,
            exchange_registry,
            kline_engine,
            orderbook_engine,
            encryption_key,
            ai_config,
            paper_mode,
            ws_broadcaster,
            proxy,
            started: AtomicBool::new(false),
            init_lock: Mutex::new(()),
            state: OnceLock::new(),
        }
    }
}

#[async_trait]
impl EngineManager for AppEngineManager {
    async fn ensure_started(&self, paper_mode: bool) -> Result<(), String> {
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
                        info!(
                            total = b.total,
                            free = b.free,
                            "Paper mode: fetched real balance as initial capital"
                        );
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
            info!("Position Engine: Real exchange mode");
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

        tokio::spawn(async move {
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
                virs_utils::crypto::derive_key(&self.encryption_key),
            ));
        let grid_llm_resolver: Arc<dyn virs_types::bot::LlmProviderResolver> =
            Arc::new(DefaultLlmResolver::new(self.ai_config.clone()));
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
        if paper_mode {
            let price_provider_for_paper: Arc<dyn PriceProvider> = grid_price_provider.clone();
            let kline_engine_for_paper = self.kline_engine.clone();
            let pe_cmd_tx_for_tick = pe_cmd_tx.clone();
            let paper_symbols_for_tick = paper_symbols.clone();
            tokio::spawn(async move {
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
                            let _ = pe_cmd_tx_for_tick
                                .send(EngineCommand::PriceTick {
                                    symbol: symbol.clone(),
                                    price,
                                })
                                .await;
                        }
                    }
                }
            });
        }

        tokio::spawn(async move {
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
                virs_utils::crypto::derive_key(&self.encryption_key),
            ));
        let auto_llm_resolver: Arc<dyn virs_types::bot::LlmProviderResolver> =
            Arc::new(DefaultLlmResolver::new(self.ai_config.clone()));
        let auto_ai_service = Arc::new(virs_bot::auto::ai::AutoAiService::new(
            auto_llm_resolver,
            auto_credential_store,
        ));

        let (mut auto_engine, auto_cmd_tx, auto_event_broadcast) = virs_bot::auto::AutoEngine::new(
            auto_store,
            auto_ai_service,
            auto_price_provider,
            auto_order_executor,
            auto_market_data_provider,
            auto_order_event_tx.clone(),
            pe_event_sender.clone(),
        );

        // Bridge AutoEvent -> WsBroadcaster
        {
            let mut auto_event_rx = auto_event_broadcast.subscribe();
            let ws_broadcaster = self.ws_broadcaster.clone();
            tokio::spawn(async move {
                loop {
                    match auto_event_rx.recv().await {
                        Ok(event) => {
                            let ws_json = match &event {
                                AutoEvent::PositionOpened {
                                    bot_id,
                                    side,
                                    price,
                                    quantity,
                                } => Some(serde_json::json!({
                                    "type": "position",
                                    "bot_id": bot_id.to_string(),
                                    "side": side,
                                    "entry_price": price,
                                    "size": quantity,
                                    "action": "opened",
                                })),
                                AutoEvent::PositionClosed {
                                    bot_id,
                                    side,
                                    price,
                                    pnl,
                                } => Some(serde_json::json!({
                                    "type": "trade",
                                    "bot_id": bot_id.to_string(),
                                    "side": side,
                                    "price": price,
                                    "pnl": pnl,
                                })),
                                AutoEvent::BotStarted { bot_id } => Some(serde_json::json!({
                                    "type": "bot_status",
                                    "bot_id": bot_id.to_string(),
                                    "status": "running",
                                })),
                                AutoEvent::BotStopped { bot_id, reason } => {
                                    Some(serde_json::json!({
                                        "type": "bot_status",
                                        "bot_id": bot_id.to_string(),
                                        "status": reason,
                                    }))
                                }
                                AutoEvent::BotError { bot_id, error } => Some(serde_json::json!({
                                    "type": "notification",
                                    "level": "error",
                                    "message": format!("Bot {}: {}", bot_id, error),
                                })),
                                _ => None,
                            };
                            if let Some(json) = ws_json {
                                ws_broadcaster.broadcast(json);
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!(lagged = n, "Auto WS event bridge lagged");
                        }
                    }
                }
            });
        }

        tokio::spawn(async move {
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
            grid_cmd_tx,
            auto_cmd_tx,
            pe_event_tx: pe_event_sender,
            position_engine: position_engine_clone,
            paper_symbols: paper_symbols.clone(),
        });
        self.started.store(true, Ordering::SeqCst);

        info!("All trading engines started successfully");
        Ok(())
    }

    fn grid_cmd_tx(&self) -> Option<mpsc::Sender<GridCommand>> {
        self.state.get().map(|s| s.grid_cmd_tx.clone())
    }

    fn auto_cmd_tx(&self) -> Option<mpsc::Sender<AutoCommand>> {
        self.state.get().map(|s| s.auto_cmd_tx.clone())
    }

    fn is_started(&self) -> bool {
        self.started.load(Ordering::SeqCst)
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
        self.state.get().map(|s| s.pe_event_tx.subscribe())
    }

    fn get_positions_by_symbol(&self, symbol: &str) -> Vec<virs_types::position::Position> {
        match self.state.get() {
            Some(s) => s
                .position_engine
                .get_all_positions()
                .into_iter()
                .filter(|p| p.symbol == symbol)
                .collect(),
            None => Vec::new(),
        }
    }

    async fn restore_if_needed(&self) {
        // Already started — nothing to do
        if self.started.load(Ordering::SeqCst) {
            return;
        }

        // Check if any bots exist in DB
        let has_bots: bool = {
            let grid_count: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM qd_grid_bots"#)
                .fetch_one(&self.db_pool)
                .await
                .unwrap_or(0);

            let auto_count: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM qd_auto_bots"#)
                .fetch_one(&self.db_pool)
                .await
                .unwrap_or(0);

            grid_count + auto_count > 0
        };

        if !has_bots {
            info!("No bots found in DB — skip restore");
            return;
        }

        info!("Bots found in DB — restoring services...");

        // 1. Restore Exchanges from DB credentials
        let rows: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
            r#"SELECT exchange, encrypted_api_key, encrypted_api_secret, encrypted_passphrase
               FROM qd_exchange_credentials"#,
        )
        .fetch_all(&self.db_pool)
        .await
        .unwrap_or_default();

        let derived_key = virs_utils::crypto::derive_key(&self.encryption_key);
        for (exchange, enc_key, enc_secret, enc_passphrase) in &rows {
            let api_key = match virs_utils::crypto::decrypt(enc_key, &derived_key) {
                Ok(k) => k,
                Err(e) => {
                    tracing::warn!(exchange, "Failed to decrypt API key: {}", e);
                    continue;
                }
            };
            let api_secret = match virs_utils::crypto::decrypt(enc_secret, &derived_key) {
                Ok(k) => k,
                Err(e) => {
                    tracing::warn!(exchange, "Failed to decrypt API secret: {}", e);
                    continue;
                }
            };
            let passphrase = enc_passphrase
                .as_ref()
                .and_then(|p| virs_utils::crypto::decrypt(p, &derived_key).ok());

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
                    info!(exchange, market_type = mt_str, "Restored exchange from DB");
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

        // 3. Determine paper mode from AppConfig (unified config consumption)
        let paper_mode = self.paper_mode.unwrap_or(true);

        // 4. Start engines (which will call restore_running_bots internally)
        if let Err(e) = self.ensure_started(paper_mode).await {
            tracing::error!("Failed to restore engines: {}", e);
        } else {
            info!(
                "Services restored successfully ({} bot symbols subscribed)",
                bot_symbols.len()
            );
        }
    }
}
