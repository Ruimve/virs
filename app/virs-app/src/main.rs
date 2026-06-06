//! VIRS App — Application entry point.
//!
//! Initializes all services, connects to the database, and starts the API server.
//! This is the composition root where all adapters are created and wired together.

mod adapters;

use std::sync::Arc;

use anyhow::Result;
use tracing::info;
use uuid::Uuid;

use virs_api::{AppState, WsBroadcaster, build_router};
use virs_bot::auto::types::AutoEvent;
use virs_bot::grid::ports as grid_ports;
use virs_bot::common::ports as common_ports;
use virs_config::load_config;
use virs_exchange::{ExchangeRegistry, PaperExchangeAdapter, CcxtExchangeAdapter};
use virs_market::{KlineEngine, ExchangeKlineSource, KlineEngineConfig};
use virs_position::{PositionEngine, Persistence as PePersistence};
use virs_types::position::EngineCommand;
use virs_types::bot::OrderEvent;
use virs_types::enums::MarketType;
use virs_types::exchange_pe::ExchangePe;

use adapters::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Load config
    let mut config = load_config()?;

    if config.server.secret_key == "change-me-to-a-random-64-char-string-in-production" {
        tracing::warn!("WARNING: Using default SECRET_KEY. Change this in production!");
    }
    if config.server.encryption_key == "change-me-to-another-random-64-char-string-must-differ-from-secret-key" {
        tracing::warn!("WARNING: Using default ENCRYPTION_KEY. Change this in production!");
    }
    if config.server.secret_key == config.server.encryption_key {
        tracing::error!("FATAL: SECRET_KEY and ENCRYPTION_KEY must be different!");
        anyhow::bail!("SECRET_KEY and ENCRYPTION_KEY must be different for security");
    }

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "virs_app=info,virs_market=info,virs_position=info,virs_bot=info,virs_api=info".into()),
        )
        .init();

    info!("VIRS starting up...");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));

    // Database connection
    let db_pool = sqlx::postgres::PgPoolOptions::new()
        .min_connections(config.database.pool_min)
        .max_connections(config.database.pool_max)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(&config.database.url)
        .await?;
    info!("Database connected");

    // Run migrations
    let init_sql = std::fs::read_to_string("migrations/init.sql")
        .or_else(|_| {
            let exe_dir = std::env::current_exe()?;
            let base = exe_dir.parent().ok_or_else(|| std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Cannot determine executable directory",
            ))?;
            std::fs::read_to_string(base.join("migrations/init.sql"))
        })
        .or_else(|_| std::fs::read_to_string("/app/migrations/init.sql"))
        .map_err(|e| anyhow::anyhow!("Failed to read migrations/init.sql: {}. Ensure the migrations directory is accessible from the working directory, next to the executable, or at /app/migrations/", e))?;
    sqlx::raw_sql(&init_sql).execute(&db_pool).await?;
    info!("Database migrations applied");

    // Create admin user if not exists
    let admin_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM qd_users WHERE username = $1)",
    )
    .bind(&config.admin.username)
    .fetch_one(&db_pool)
    .await?;

    let admin_id: Uuid = if !admin_exists {
        let password_hash = bcrypt::hash(&config.admin.password, bcrypt::DEFAULT_COST)?;
        let row: (Uuid,) = sqlx::query_as(
            "INSERT INTO qd_users (username, password_hash, role, is_active, credits) VALUES ($1, $2, 'admin', true, 999999) RETURNING id",
        )
        .bind(&config.admin.username)
        .bind(password_hash)
        .fetch_one(&db_pool)
        .await?;
        info!("Admin user '{}' created (id={})", config.admin.username, row.0);
        row.0
    } else {
        let row: (Uuid,) = sqlx::query_as(
            "SELECT id FROM qd_users WHERE username = $1 AND role = 'admin' LIMIT 1",
        )
        .bind(&config.admin.username)
        .fetch_one(&db_pool)
        .await?;
        row.0
    };
    config.admin.id = Some(admin_id);

    // Paper mode
    let paper_mode = config.paper.unwrap_or(true);
    info!("Paper mode: {}", paper_mode);

    // Create exchange registry
    let exchange_registry = Arc::new(ExchangeRegistry::new());

    // Register exchange instances from DB credentials
    {
        let rows_result = sqlx::query_as::<_, (String, String, String, Option<String>)>(
            r#"SELECT exchange, encrypted_api_key, encrypted_api_secret, encrypted_passphrase
               FROM qd_exchange_credentials"#,
        )
        .fetch_all(&db_pool)
        .await;

        let rows = match rows_result {
            Ok(r) => { info!("Loaded {} credential rows from DB", r.len()); r }
            Err(e) => { tracing::error!(error = %e, "Failed to load credentials from DB"); Vec::new() }
        };

        let derived_key = virs_utils::crypto::derive_key(&config.server.encryption_key);
        for (exchange, enc_key, enc_secret, enc_passphrase) in &rows {
            let api_key = match virs_utils::crypto::decrypt(enc_key, &derived_key) {
                Ok(k) => k,
                Err(e) => { tracing::warn!(exchange = %exchange, error = %e, "Failed to decrypt api_key"); continue }
            };
            let api_secret = match virs_utils::crypto::decrypt(enc_secret, &derived_key) {
                Ok(s) => s,
                Err(e) => { tracing::warn!(exchange = %exchange, error = %e, "Failed to decrypt api_secret"); continue }
            };
            let passphrase = enc_passphrase.as_ref().and_then(|p| virs_utils::crypto::decrypt(p, &derived_key).ok());

            // Register perpetual (no proxy — Docker containers may not reach host proxy)
            match virs_ccxt::create_exchange(
                exchange, &api_key, &api_secret, passphrase.as_deref(), None, &virs_ccxt::MarketType::Perpetual,
            ) {
                Ok(ccxt_ex) => {
                    let adapter = virs_exchange::CcxtAdapter::new(ccxt_ex, MarketType::Perpetual);
                    exchange_registry.register(Box::new(adapter));
                }
                Err(e) => {
                    tracing::warn!(exchange = %exchange, error = %e, "Failed to create perpetual exchange instance");
                }
            }

            // Register spot
            match virs_ccxt::create_exchange(
                exchange, &api_key, &api_secret, passphrase.as_deref(), None, &virs_ccxt::MarketType::Spot,
            ) {
                Ok(ccxt_ex) => {
                    let adapter = virs_exchange::CcxtAdapter::new(ccxt_ex, MarketType::Spot);
                    exchange_registry.register(Box::new(adapter));
                }
                Err(e) => {
                    tracing::warn!(exchange = %exchange, error = %e, "Failed to create spot exchange instance");
                }
            }
        }
        info!("Exchange registry initialized with {} entries", exchange_registry.registered_names().len());
    }

    // WebSocket broadcaster
    let ws_broadcaster = Arc::new(WsBroadcaster::new());

    // ── Kline Engine ──
    let kline_config = KlineEngineConfig {
        proxy_url: config.proxy.clone(),
        ..Default::default()
    };
    let kline_source = Arc::new(ExchangeKlineSource::new(exchange_registry.clone()));
    let spot_ws = Arc::new(tokio::sync::Mutex::new(
        virs_ccxt::adapter::binance::kline_ws::BinanceKlineWs::new_spot(config.proxy.as_deref()),
    ));
    let perpetual_ws = Arc::new(tokio::sync::Mutex::new(
        virs_ccxt::adapter::binance::kline_ws::BinanceKlineWs::new_perpetual(config.proxy.as_deref()),
    ));
    let kline_engine = Arc::new(KlineEngine::new(kline_config, kline_source, spot_ws, perpetual_ws));
    kline_engine.start().await;
    info!("Kline engine started");

    // ── Position Engine ──
    let pe_exchange: Box<dyn ExchangePe> = if paper_mode {
        let initial_balance = {
            let temp_adapter = CcxtExchangeAdapter::new(exchange_registry.clone());
            match temp_adapter.get_balance().await {
                Ok(b) => {
                    info!(total = b.total, free = b.free, "Paper mode: fetched real exchange balance as initial capital");
                    b.total
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Paper mode: failed to fetch real balance, using 0 as initial capital");
                    0.0
                }
            }
        };
        Box::new(
            PaperExchangeAdapter::new("paper", MarketType::Perpetual, initial_balance)
                .with_exchange_registry(exchange_registry.clone()),
        )
    } else {
        info!("Position Engine: Real exchange mode");
        Box::new(CcxtExchangeAdapter::new(exchange_registry.clone()))
    };

    let pe_persistence = Box::new(PePersistence::new(db_pool.clone()));
    let pe_config = virs_types::position::EngineConfig::default();

    let mut position_engine = PositionEngine::new(pe_config, pe_exchange, pe_persistence);
    let pe_cmd_tx = position_engine.command_sender();
    let grid_pe_event_rx = position_engine.subscribe_events();
    let auto_pe_event_rx = position_engine.subscribe_events();
    let pe_exchange_ref = position_engine.exchange();

    tokio::spawn(async move {
        if let Err(e) = position_engine.run().await {
            tracing::error!(error = %e, "Position Engine run failed");
        }
    });
    info!("Position Engine started (paper={})", paper_mode);

    // ── Grid Engine ──
    let (grid_event_tx, _grid_event_rx) = tokio::sync::broadcast::channel(256);

    let grid_store = Arc::new(PgGridStore::new(db_pool.clone()));
    let grid_price_provider = Arc::new(ExchangePriceProvider::new(exchange_registry.clone())
        .with_kline_engine(kline_engine.clone())
        .with_db(db_pool.clone(), config.server.encryption_key.clone()));
    let grid_market_data_provider = Arc::new(ExchangeMarketDataProvider::new(exchange_registry.clone())
        .with_kline_engine(kline_engine.clone())
        .with_pe_exchange(pe_exchange_ref.clone()));
    let grid_order_executor = Arc::new(PeOrderExecutor::new(
        pe_cmd_tx.clone(),
        grid_event_tx.clone(),
        grid_pe_event_rx,
    ));
    let grid_credential_store: Arc<dyn common_ports::CredentialStore> = Arc::new(PgCredentialStore::new(
        db_pool.clone(),
        virs_utils::crypto::derive_key(&config.server.encryption_key),
    ));
    let grid_llm_resolver: Arc<dyn common_ports::LlmProviderResolver> = Arc::new(DefaultLlmResolver::new(config.ai.clone()));
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

    // Paper mode price tick — drive PaperExchangeAdapter Limit order matching
    if paper_mode {
        let price_provider_for_paper: Arc<dyn grid_ports::PriceProvider> = grid_price_provider.clone();
        let kline_engine_for_paper = kline_engine.clone();
        let pe_cmd_tx_for_tick = pe_cmd_tx.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(3));
            loop {
                tick.tick().await;
                for (exchange, symbol, _market_type) in kline_engine_for_paper.subscribed_symbols() {
                    if let Some(price) = price_provider_for_paper.get_price(&exchange, &symbol).await {
                        let _ = pe_cmd_tx_for_tick.send(EngineCommand::PriceTick {
                            symbol: symbol.clone(),
                            price,
                        }).await;
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
    let auto_store = Arc::new(PgAutoStore::new(db_pool.clone()));
    let auto_price_provider = Arc::new(AutoExchangePriceProvider::new(exchange_registry.clone())
        .with_kline_engine(kline_engine.clone())
        .with_db(db_pool.clone(), config.server.encryption_key.clone()));
    let auto_market_data_provider = Arc::new(AutoExchangeMarketDataProvider::new(exchange_registry.clone())
        .with_kline_engine(kline_engine.clone())
        .with_db(db_pool.clone(), config.server.encryption_key.clone())
        .with_pe_exchange(pe_exchange_ref.clone()));
    let (auto_order_event_tx, _) = tokio::sync::broadcast::channel::<OrderEvent>(256);
    let auto_order_executor = Arc::new(PeOrderExecutor::new(
        pe_cmd_tx.clone(),
        auto_order_event_tx.clone(),
        auto_pe_event_rx,
    ));
    let auto_credential_store: Arc<dyn common_ports::CredentialStore> = Arc::new(PgCredentialStore::new(
        db_pool.clone(),
        virs_utils::crypto::derive_key(&config.server.encryption_key),
    ));
    let auto_llm_resolver: Arc<dyn common_ports::LlmProviderResolver> = Arc::new(DefaultLlmResolver::new(config.ai.clone()));
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
    );

    // Bridge AutoEvent -> WsBroadcaster
    {
        let mut auto_event_rx = auto_event_broadcast.subscribe();
        let ws_broadcaster = ws_broadcaster.clone();
        tokio::spawn(async move {
            loop {
                match auto_event_rx.recv().await {
                    Ok(event) => {
                        let ws_json = match &event {
                            AutoEvent::PriceUpdate {
                                bot_id, symbol, side, entry_price, position_size,
                                current_price, unrealized_pnl, total_pnl, liquidation_price,
                            } => Some(serde_json::json!({
                                "type": "position_pnl",
                                "bot_id": bot_id.to_string(),
                                "symbol": symbol,
                                "side": side,
                                "entry_price": entry_price,
                                "position_size": position_size,
                                "current_price": current_price,
                                "unrealized_pnl": unrealized_pnl,
                                "total_pnl": total_pnl,
                                "liquidation_price": liquidation_price,
                            })),
                            AutoEvent::PositionOpened { bot_id, side, price, quantity } => {
                                Some(serde_json::json!({
                                    "type": "position",
                                    "bot_id": bot_id.to_string(),
                                    "side": side,
                                    "entry_price": price,
                                    "size": quantity,
                                    "action": "opened",
                                }))
                            }
                            AutoEvent::PositionClosed { bot_id, side, price, pnl } => {
                                Some(serde_json::json!({
                                    "type": "trade",
                                    "bot_id": bot_id.to_string(),
                                    "side": side,
                                    "price": price,
                                    "pnl": pnl,
                                }))
                            }
                            AutoEvent::BotStarted { bot_id } => {
                                Some(serde_json::json!({
                                    "type": "bot_status",
                                    "bot_id": bot_id.to_string(),
                                    "status": "running",
                                }))
                            }
                            AutoEvent::BotStopped { bot_id, reason } => {
                                Some(serde_json::json!({
                                    "type": "bot_status",
                                    "bot_id": bot_id.to_string(),
                                    "status": reason,
                                }))
                            }
                            AutoEvent::BotError { bot_id, error } => {
                                Some(serde_json::json!({
                                    "type": "notification",
                                    "level": "error",
                                    "message": format!("Bot {}: {}", bot_id, error),
                                }))
                            }
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

    // Build app state
    let app_state = AppState {
        db_pool: db_pool.clone(),
        ws_broadcaster,
        grid_cmd_tx: Some(grid_cmd_tx),
        auto_cmd_tx: Some(auto_cmd_tx),
        paper_mode,
        http_client: reqwest::Client::new(),
        exchange_registry: exchange_registry.clone(),
        kline_engine: kline_engine.clone(),
        encryption_key: config.server.encryption_key.clone(),
    };

    // Build router
    let app = build_router(app_state);

    // Start server
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("API server listening on http://{}", addr);

    axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("VIRS shut down gracefully");
    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C handler");
    info!("Received shutdown signal");
}
