//! VIRS App — Application entry point.
//!
//! Initializes the API server with lazy engine startup.
//! Trading engines (Position, Grid, Auto) are NOT started at boot.
//! They are started when the first bot is created after the wizard,
//! using the exchange credentials provided by the user.

mod adapters;
mod engine_manager;

use std::sync::Arc;

use anyhow::Result;
use tracing::info;
use uuid::Uuid;
use virs_api::EngineManager;

use virs_api::{build_router, AppState, WsBroadcaster};
use virs_config::load_config;
use virs_exchange::Exchanges;
use virs_market::{
    ExchangeKlineSource, KlineEngine, KlineEngineConfig, OrderBookEngine, OrderBookEngineConfig,
};

use engine_manager::AppEngineManager;

#[tokio::main]
async fn main() -> Result<()> {
    // Load config
    let mut config = load_config()?;

    if config.server.secret_key == "change-me-to-a-random-64-char-string-in-production" {
        tracing::warn!("WARNING: Using default SECRET_KEY. Change this in production!");
    }
    if config.server.encryption_key
        == "change-me-to-another-random-64-char-string-must-differ-from-secret-key"
    {
        tracing::warn!("WARNING: Using default ENCRYPTION_KEY. Change this in production!");
    }
    if config.server.secret_key == config.server.encryption_key {
        tracing::error!("FATAL: SECRET_KEY and ENCRYPTION_KEY must be different!");
        anyhow::bail!("SECRET_KEY and ENCRYPTION_KEY must be different for security");
    }

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "virs_app=info,virs_market=info,virs_position=info,virs_bot=info,virs_api=info"
                    .into()
            }),
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
    let admin_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM qd_users WHERE username = $1)")
            .bind(&config.admin.username)
            .fetch_one(&db_pool)
            .await?;

    let admin_id: Uuid = if !admin_exists {
        let password_hash = virs_utils::crypto::hash_password(&config.admin.password)?;
        let row: (Uuid,) = sqlx::query_as(
            "INSERT INTO qd_users (username, password_hash, role, is_active) VALUES ($1, $2, 'admin', true) RETURNING id",
        )
        .bind(&config.admin.username)
        .bind(password_hash)
        .fetch_one(&db_pool)
        .await?;
        info!(
            "Admin user '{}' created (id={})",
            config.admin.username, row.0
        );
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

    // Create exchange registry (empty — populated when user saves credentials)
    let exchange_registry = Arc::new(Exchanges::new());
    info!("Exchange registry initialized (empty — will be populated on first credential save)");

    // WebSocket broadcaster
    let ws_broadcaster = Arc::new(WsBroadcaster::new());

    // ── Kline Engine ──
    // Created at boot but only subscribes when bots need data
    let kline_config = KlineEngineConfig {
        proxy_url: config.proxy.clone(),
        ..Default::default()
    };
    let kline_source = Arc::new(ExchangeKlineSource::new(exchange_registry.clone()));
    let spot_ws = Arc::new(tokio::sync::Mutex::new(
        virs_ccxt::adapter::binance::kline_ws::BinanceKlineWs::new_spot(config.proxy.as_deref()),
    ));
    let perpetual_ws = Arc::new(tokio::sync::Mutex::new(
        virs_ccxt::adapter::binance::kline_ws::BinanceKlineWs::new_perpetual(
            config.proxy.as_deref(),
        ),
    ));
    let kline_engine = Arc::new(KlineEngine::new(
        kline_config,
        kline_source,
        spot_ws,
        perpetual_ws,
    ));
    info!("Kline engine created (lazy — will start on first subscribe)");

    // ── OrderBook Engine ──
    // Created at boot but only subscribes when bots need data
    let ob_spot_ws = Arc::new(tokio::sync::Mutex::new(
        virs_ccxt::adapter::binance::orderbook_ws::BinanceOrderBookWs::new_spot(
            config.proxy.as_deref(),
        ),
    ));
    let ob_perpetual_ws = Arc::new(tokio::sync::Mutex::new(
        virs_ccxt::adapter::binance::orderbook_ws::BinanceOrderBookWs::new_perpetual(
            config.proxy.as_deref(),
        ),
    ));
    let orderbook_engine = Arc::new(OrderBookEngine::new(
        OrderBookEngineConfig::default(),
        ob_spot_ws,
        ob_perpetual_ws,
    ));
    info!("OrderBook engine created (lazy — will start on first subscribe)");

    // ── Engine Manager (lazy) ──
    // Position/Grid/Auto engines are NOT started here.
    // They will be started when the first bot is created via API.
    let engine_manager = Arc::new(AppEngineManager::new(
        db_pool.clone(),
        exchange_registry.clone(),
        kline_engine.clone(),
        orderbook_engine.clone(),
        config.server.encryption_key.clone(),
        config.ai.clone(),
        config.paper,
        ws_broadcaster.clone(),
        config.proxy.clone(),
    ));
    info!("Engine manager created (engines will start on first bot creation)");

    // Build app state
    let app_state = AppState {
        db_pool: db_pool.clone(),
        ws_broadcaster,
        engine_manager: engine_manager.clone(),
        http_client: reqwest::Client::new(),
        exchange_registry: exchange_registry.clone(),
        kline_engine: kline_engine.clone(),
        orderbook_engine: orderbook_engine.clone(),
        encryption_key: config.server.encryption_key.clone(),
    };

    // Restore services if bots exist from previous session
    engine_manager.restore_if_needed().await;

    // Build router
    let app = build_router(app_state);

    // Start server
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("API server listening on http://{}", addr);

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
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
