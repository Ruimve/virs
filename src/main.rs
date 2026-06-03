use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod api;
mod bot;
mod config;
mod engine;
mod indicators;
mod models;
mod services;
mod trading;
mod utils;

use config::load_config;
use trading::exchange::registry::ExchangeRegistry;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| config.server.log_level.clone().into()),
        )
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();

    info!("🚀 VIRS starting up...");
    info!("📦 Version: {}", env!("CARGO_PKG_VERSION"));

    // Connect to database
    let db_pool = sqlx::postgres::PgPoolOptions::new()
        .min_connections(config.database.pool_min)
        .max_connections(config.database.pool_max)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(&config.database.url)
        .await?;

    info!("✅ Database connected");

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
        .or_else(|_| {
            std::fs::read_to_string("/app/migrations/init.sql")
        })
        .map_err(|e| anyhow::anyhow!("Failed to read migrations/init.sql: {}. Ensure the migrations directory is accessible from the working directory, next to the executable, or at /app/migrations/", e))?;
    sqlx::raw_sql(&init_sql).execute(&db_pool).await?;
    info!("✅ Database migrations applied");

    // Create admin user if not exists
    let admin_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM qd_users WHERE username = $1)",
    )
    .bind(&config.admin.username)
    .fetch_one(&db_pool)
    .await?;

    // Create admin user if not exists, and record the UUID for protection
    let admin_id: uuid::Uuid = if !admin_exists {
        let password_hash = bcrypt::hash(&config.admin.password, bcrypt::DEFAULT_COST)?;
        let row: (uuid::Uuid,) = sqlx::query_as(
            "INSERT INTO qd_users (username, password_hash, role, is_active, credits) VALUES ($1, $2, 'admin', true, 999999) RETURNING id",
        )
        .bind(&config.admin.username)
        .bind(password_hash)
        .fetch_one(&db_pool)
        .await?;
        info!("✅ Admin user '{}' created (id={})", config.admin.username, row.0);
        row.0
    } else {
        let row: (uuid::Uuid,) = sqlx::query_as(
            "SELECT id FROM qd_users WHERE username = $1 AND role = 'admin' LIMIT 1",
        )
        .bind(&config.admin.username)
        .fetch_one(&db_pool)
        .await?;
        row.0
    };
    config.admin.id = Some(admin_id);

    // Create exchange registry
    let exchange_registry = Arc::new(ExchangeRegistry::new());

    // Create WebSocket broadcaster for real-time push
    let ws_broadcaster = Arc::new(api::ws::create_broadcaster(256));

    info!("ℹ️  Exchange credentials loaded on-demand from database (Credentials page)");

    // Create kline engine
    let kline_config = engine::kline::types::KlineEngineConfig {
        proxy_url: config.proxy.clone(),
        ..Default::default()
    };
    let kline_source = trading::exchange::ExchangeFactory::create_kline_source(config.proxy.clone());
    let spot_ws = trading::exchange::ExchangeFactory::create_binance_kline_ws(models::MarketType::Spot, config.proxy.as_deref());
    let perpetual_ws = trading::exchange::ExchangeFactory::create_binance_kline_ws(models::MarketType::Perpetual, config.proxy.as_deref());
    let kline_engine = std::sync::Arc::new(engine::kline::KlineEngine::new(kline_config, kline_source, spot_ws, perpetual_ws));
    kline_engine.start().await;
    info!("✅ Kline engine started");

    // Start Grid Engine
    let (pe_cmd_tx, _pe_cmd_rx) = tokio::sync::mpsc::channel(256);
    let (_pe_event_tx, pe_event_rx) = tokio::sync::broadcast::channel(256);
    let (grid_event_tx, _grid_event_rx) = tokio::sync::broadcast::channel(256);

    // Paper 交易执行器
    let paper_executor = Arc::new(trading::paper::PaperOrderExecutor::new(grid_event_tx.clone()));
    let paper_for_tick = paper_executor.clone();

    // Adapter 实现
    let grid_store = Arc::new(bot::semi_automatic_grid::adapters::PgGridStore::new(db_pool.clone()));
    let grid_price_provider: Arc<dyn bot::semi_automatic_grid::ports::PriceProvider> =
        Arc::new(bot::semi_automatic_grid::adapters::ExchangePriceProvider::new(exchange_registry.clone())
            .with_kline_engine(kline_engine.clone()));
    let grid_market_data_provider: Arc<dyn bot::semi_automatic_grid::ports::MarketDataProvider> =
        Arc::new(bot::semi_automatic_grid::adapters::ExchangeMarketDataProvider::new(exchange_registry.clone())
            .with_kline_engine(kline_engine.clone()));
    let real_order_executor: Arc<dyn bot::semi_automatic_grid::ports::OrderExecutor> =
        Arc::new(bot::semi_automatic_grid::adapters::PeOrderExecutor::new(pe_cmd_tx.clone(), exchange_registry.clone(), "grid:close"));
    let grid_order_executor: Arc<dyn bot::semi_automatic_grid::ports::OrderExecutor> =
        Arc::new(bot::semi_automatic_grid::adapters::SwitchableOrderExecutor::new(
            real_order_executor,
            paper_executor.clone(),
        ));
    let grid_credential_store: Box<dyn bot::semi_automatic_grid::ports::CredentialStore> =
        Box::new(bot::semi_automatic_grid::adapters::PgCredentialStore::new(
            db_pool.clone(),
            utils::crypto::derive_key(&config.server.encryption_key),
        ));
    let grid_llm_resolver: Box<dyn bot::semi_automatic_grid::ports::LlmProviderResolver> =
        Box::new(bot::semi_automatic_grid::adapters::DefaultLlmResolver::new(config.ai.clone()));
    let grid_ai_service = Arc::new(bot::semi_automatic_grid::ai::GridAiService::new(
        grid_llm_resolver,
        grid_credential_store,
    ));

    let grid_price_provider_for_engine = grid_price_provider.clone();

    let (mut grid_engine, grid_cmd_tx, _grid_event_broadcast) = bot::semi_automatic_grid::GridEngine::new(
        grid_store,
        grid_ai_service,
        grid_price_provider_for_engine,
        grid_order_executor,
        grid_market_data_provider,
        grid_event_tx.clone(),
        Some(kline_engine.clone()),
    );

    // PE 事件转换 bridge
    let mut pe_event_rx_for_bridge = pe_event_rx;
    let grid_event_tx_for_bridge = grid_event_tx.clone();
    tokio::spawn(async move {
        loop {
            match pe_event_rx_for_bridge.recv().await {
                Ok(event) => {
                    if let Some(grid_event) = bot::semi_automatic_grid::adapters::convert_pe_event(event) {
                        let _ = grid_event_tx_for_bridge.send(grid_event);
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(lagged = n, "PE event bridge lagged");
                }
            }
        }
    });

    // Paper 交易 price tick 协程
    let price_provider_for_paper = grid_price_provider;
    let kline_engine_for_paper = kline_engine.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(3));
        loop {
            tick.tick().await;
            if !paper_for_tick.is_enabled() {
                continue;
            }
            for (exchange, symbol, _market_type) in kline_engine_for_paper.subscribed_symbols() {
                if let Some(price) = price_provider_for_paper.get_price(&exchange, &symbol).await {
                    paper_for_tick.on_price_tick(&symbol, price).await;
                }
            }
        }
    });

    let grid_cmd_tx = Some(grid_cmd_tx);
    tokio::spawn(async move {
        grid_engine.run().await;
    });
    info!("✅ Grid engine started (paper trading available)");

    // Start Auto Trade Engine
    let auto_store = Arc::new(bot::auto_trade::adapters::PgAutoStore::new(db_pool.clone()));
    let auto_price_provider: Arc<dyn bot::auto_trade::ports::PriceProvider> =
        Arc::new(bot::auto_trade::adapters::ExchangePriceProvider::new(exchange_registry.clone())
            .with_kline_engine(kline_engine.clone()));
    let auto_market_data_provider: Arc<dyn bot::auto_trade::ports::MarketDataProvider> =
        Arc::new(bot::auto_trade::adapters::ExchangeMarketDataProvider::new(exchange_registry.clone())
            .with_kline_engine(kline_engine.clone())
            .with_db(db_pool.clone(), config.server.encryption_key.clone())
            .with_paper_executor(paper_executor.clone()));
    let auto_real_order_executor: Arc<dyn bot::auto_trade::ports::OrderExecutor> =
        Arc::new(bot::auto_trade::adapters::PeOrderExecutor::new(pe_cmd_tx.clone(), exchange_registry.clone(), "auto:close"));
    let auto_order_executor: Arc<dyn bot::auto_trade::ports::OrderExecutor> =
        Arc::new(bot::auto_trade::adapters::SwitchableOrderExecutor::new(
            auto_real_order_executor,
            paper_executor.clone(),
        ));
    let auto_credential_store: Box<dyn bot::auto_trade::ports::CredentialStore> =
        Box::new(bot::auto_trade::adapters::PgCredentialStore::new(
            db_pool.clone(),
            utils::crypto::derive_key(&config.server.encryption_key),
        ));
    let auto_llm_resolver: Box<dyn bot::auto_trade::ports::LlmProviderResolver> =
        Box::new(bot::auto_trade::adapters::DefaultLlmResolver::new(config.ai.clone()));
    let auto_ai_service = Arc::new(bot::auto_trade::ai::AutoAiService::new(
        auto_llm_resolver,
        auto_credential_store,
    ));

    let (auto_event_tx, _auto_event_rx) = tokio::sync::broadcast::channel::<bot::auto_trade::ports::OrderEvent>(256);

    {
        paper_executor.add_event_channel(auto_event_tx.clone()).await;
    }

    let (mut auto_engine, auto_cmd_tx, auto_event_broadcast) = bot::auto_trade::AutoEngine::new(
        auto_store,
        auto_ai_service,
        auto_price_provider,
        auto_order_executor,
        auto_market_data_provider,
        auto_event_tx.clone(),
        Some(kline_engine.clone()),
    );

    let mut auto_pe_event_rx = _pe_event_tx.subscribe();
    let auto_event_tx_for_bridge = auto_event_tx;
    tokio::spawn(async move {
        loop {
            match auto_pe_event_rx.recv().await {
                Ok(event) => {
                    if let Some(auto_event) = bot::auto_trade::adapters::convert_pe_event(event) {
                        let _ = auto_event_tx_for_bridge.send(auto_event);
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(lagged = n, "Auto PE event bridge lagged");
                }
            }
        }
    });

    // 桥接 AutoEvent → WsBroadcaster，推送实时仓位盈亏到前端
    {
        let mut auto_event_rx = auto_event_broadcast.subscribe();
        let ws_broadcaster = ws_broadcaster.clone();
        tokio::spawn(async move {
            loop {
                match auto_event_rx.recv().await {
                    Ok(event) => {
                        let ws_event = match &event {
                            bot::auto_trade::types::AutoEvent::PriceUpdate {
                                bot_id, symbol, side, entry_price, position_size,
                                current_price, unrealized_pnl, total_pnl, liquidation_price,
                            } => Some(api::ws::WsEvent::PositionPnl {
                                bot_id: bot_id.to_string(),
                                symbol: symbol.clone(),
                                side: side.clone(),
                                entry_price: *entry_price,
                                position_size: *position_size,
                                current_price: *current_price,
                                unrealized_pnl: *unrealized_pnl,
                                total_pnl: *total_pnl,
                                liquidation_price: *liquidation_price,
                            }),
                            bot::auto_trade::types::AutoEvent::PositionOpened { bot_id, side, price, quantity } => {
                                Some(api::ws::WsEvent::Position {
                                    bot_id: bot_id.to_string(),
                                    symbol: String::new(),
                                    side: side.clone(),
                                    size: *quantity,
                                    entry_price: *price,
                                    action: "opened".to_string(),
                                })
                            }
                            bot::auto_trade::types::AutoEvent::PositionClosed { bot_id, side, price, pnl } => {
                                Some(api::ws::WsEvent::Trade {
                                    bot_id: bot_id.to_string(),
                                    symbol: String::new(),
                                    side: side.clone(),
                                    price: *price,
                                    amount: 0.0,
                                    pnl: *pnl,
                                })
                            }
                            bot::auto_trade::types::AutoEvent::BotStarted { bot_id } => {
                                Some(api::ws::WsEvent::BotStatus {
                                    bot_id: bot_id.to_string(),
                                    name: String::new(),
                                    status: "running".to_string(),
                                })
                            }
                            bot::auto_trade::types::AutoEvent::BotStopped { bot_id, reason } => {
                                Some(api::ws::WsEvent::BotStatus {
                                    bot_id: bot_id.to_string(),
                                    name: String::new(),
                                    status: reason.clone(),
                                })
                            }
                            bot::auto_trade::types::AutoEvent::BotError { bot_id, error } => {
                                Some(api::ws::WsEvent::Notification {
                                    level: "error".to_string(),
                                    message: format!("Bot {}: {}", bot_id, error),
                                })
                            }
                            _ => None,
                        };
                        if let Some(ws_event) = ws_event {
                            let _ = ws_broadcaster.send(ws_event);
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

    let auto_cmd_tx = Some(auto_cmd_tx);
    tokio::spawn(async move {
        auto_engine.run().await;
    });
    info!("✅ Auto trade engine started");

    // Build and start HTTP server
    let app = api::build_router(
        Arc::new(config.clone()),
        exchange_registry,
        db_pool,
        ws_broadcaster,
        Some(kline_engine),
        grid_cmd_tx,
        auto_cmd_tx,
        Some(paper_executor),
    );

    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("🌐 API server listening on http://{}", addr);

    axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("👋 VIRS shut down gracefully");
    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C handler");
    info!("Received shutdown signal");
}
