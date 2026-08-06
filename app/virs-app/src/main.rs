use std::sync::Arc;

use tracing::{error, info};
use uuid::Uuid;
use virs_api::EngineManager;
use virs_error::{Context, VirsResult};

use virs_api::{build_router, AppState};
use virs_app::AppEngineManager;
use virs_config::load_config;
use virs_exchange::Exchanges;
use virs_market::{
    ExchangeKlineSource, KlineEngine, KlineEngineConfig, OrderBookEngine, OrderBookEngineConfig,
};

#[tokio::main]
async fn main() -> VirsResult<()> {
    let mut config = load_config()?;

    if config.server.encryption_key == "change-me-to-a-random-64-char-string-in-production" {
        error!(key = "ENCRYPTION_KEY", "Using default key — change in production");
    }
    if config.server.llm_key == "change-me-to-a-random-64-char-string-in-production" {
        error!(key = "LLM_KEY", "Using default key — change in production");
    }
    if config.server.jwt_secret == "change-me-to-a-random-32-char-or-longer-string-in-production" {
        error!(key = "JWT_SECRET", "Using default key — change in production");
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "virs_app=info,virs_trading_bot=info,virs_position=info,virs_market=info,virs_api=info"
                    .into()
            }),
        )
        .init();

    info!(version = env!("CARGO_PKG_VERSION"), "VIRS starting up");

    let db_pool = sqlx::postgres::PgPoolOptions::new()
        .min_connections(config.database.pool_min)
        .max_connections(config.database.pool_max)
        .acquire_timeout(std::time::Duration::from_secs(
            config.database.acquire_timeout_secs,
        ))
        .connect(&config.database.url)
        .await?;
    info!("Database connected");

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
        .context("Failed to read migrations/init.sql: ensure the migrations directory is accessible from the working directory, next to the executable, or at /app/migrations/")?;
    sqlx::raw_sql(&init_sql).execute(&db_pool).await?;
    info!("Database migrations applied");

    let admin_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM qd_users WHERE username = $1)")
            .bind(&config.admin.username)
            .fetch_one(&db_pool)
            .await?;

    let admin_id: Uuid = if !admin_exists {
        let password_hash = virs_utils::hash_password(&config.admin.password)?;
        let row: (Uuid,) = sqlx::query_as(
            "INSERT INTO qd_users (username, password_hash, role, is_active) VALUES ($1, $2, 'admin', true) RETURNING id",
        )
        .bind(&config.admin.username)
        .bind(password_hash)
        .fetch_one(&db_pool)
        .await?;
        info!(
            username = %config.admin.username,
            admin_id = %row.0,
            "Admin user created"
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

    let exchange_registry = Arc::new(Exchanges::new());
    info!("Exchange registry initialized (empty — will be populated on first credential save)");

    let kline_config = KlineEngineConfig {
        proxy_url: config.proxy.clone(),
        ..Default::default()
    };
    let kline_source = Arc::new(ExchangeKlineSource::new(exchange_registry.clone()));
    let perpetual_ws = virs_ccxt::create_kline_ws(config.proxy.as_deref());
    let kline_engine = Arc::new(KlineEngine::new(
        kline_config,
        kline_source,
        perpetual_ws,
    ));
    info!("Kline engine created (lazy — will start on first subscribe)");

    let ob_perpetual_ws = virs_ccxt::create_orderbook_ws(config.proxy.as_deref());
    let orderbook_engine = Arc::new(OrderBookEngine::new(
        OrderBookEngineConfig::default(),
        ob_perpetual_ws,
    ));
    info!("OrderBook engine created (lazy — will start on first subscribe)");

    let prompt_loader = virs_prompt::PromptLoader::from_env().await;

    let engine_manager = Arc::new(AppEngineManager::new(
        db_pool.clone(),
        exchange_registry.clone(),
        kline_engine.clone(),
        orderbook_engine.clone(),
        config.server.encryption_key.clone(),
        config.server.llm_key.clone(),
        config.proxy.clone(),
        config.time.clone(),
        prompt_loader.clone(),
    ));
    info!("Engine manager created (engines will start on first bot creation)");

    let app_state = AppState {
        db_pool: db_pool.clone(),
        engine_manager: engine_manager.clone(),
        http_client: reqwest::Client::new(),
        exchange_registry: exchange_registry.clone(),
        kline_engine: kline_engine.clone(),
        orderbook_engine: orderbook_engine.clone(),
        encryption_key: config.server.encryption_key.clone(),
        llm_key: config.server.llm_key.clone(),
        jwt_secret: config.server.jwt_secret.clone(),
        jwt_expiration_hours: config.server.jwt_expiration_hours,
        http_timeout_secs: config.time.http_timeout_secs,
        http_connect_timeout_secs: config.time.http.http_connect_timeout_secs,
        http_pool_max_idle_per_host: config.time.http.http_pool_max_idle_per_host,
        listenkey_keepalive_futures_secs: config.time.listenkey.listenkey_keepalive_futures_secs,
        prompt_loader,
    };

    let _ = engine_manager.restore_if_needed().await;

    let app = build_router(app_state);

    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .context(format!("Failed to bind to {}", addr))?;
    info!(addr = %addr, "API server listening");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .context("axum server error")?;

    info!("Shutting down trading engines...");
    engine_manager.shutdown().await;

    info!("Shutting down market data engines...");
    kline_engine.stop().await;
    orderbook_engine.stop().await;

    info!("VIRS shut down gracefully");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("Received SIGINT (Ctrl+C)"),
        _ = terminate => info!("Received SIGTERM"),
    }
}
