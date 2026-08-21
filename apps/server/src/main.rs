use std::sync::Arc;

use tracing::{error, info};
use virs_api::EngineManager;
use virs_error::{Context, VirsResult};

use virs_api::{build_router, AppState};
use server::AppEngineManager;
use virs_config::load_config;
use virs_database::{create_pool, ensure_admin, run_migrations};
use virs_exchange::Exchanges;
use virs_market::{
    create_exchange_kline_source, create_kline_engine, create_orderbook_engine,
    KlineEngineConfig, OrderBookEngineConfig,
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
                "server=info,virs_trading_bot=info,virs_position=info,virs_market=info,virs_api=info"
                    .into()
            }),
        )
        .init();

    info!(version = env!("CARGO_PKG_VERSION"), "VIRS starting up");

    let db_pool = create_pool(
        &config.database.url,
        config.database.pool_min,
        config.database.pool_max,
        std::time::Duration::from_secs(config.database.acquire_timeout_secs),
    )
    .await?;
    info!("Database connected");

    run_migrations(&db_pool).await?;
    info!("Database migrations applied");

    let password_hash = virs_utils::hash_password(&config.admin.password)?;
    let admin_id = ensure_admin(&db_pool, &config.admin.username, &password_hash).await?;
    info!(
        username = %config.admin.username,
        admin_id = %admin_id,
        "Admin user ensured"
    );
    config.admin.id = Some(admin_id);

    let exchange_registry = Arc::new(Exchanges::new());
    info!("Exchange registry initialized (empty — will be populated on first credential save)");

    /* App层装配：创建具体实现并强制转换为trait object（Arc<dyn KlineEngineHandle>等），
     * 供上层以trait抽象方式使用，实现依赖倒置。 */
    let kline_config = KlineEngineConfig {
        proxy_url: config.proxy.clone(),
        ..Default::default()
    };
    let kline_source = create_exchange_kline_source(exchange_registry.clone());
    let perpetual_ws = virs_ccxt::create_kline_ws(config.proxy.as_deref());
    let kline_engine = create_kline_engine(
        kline_config,
        kline_source,
        perpetual_ws,
    );
    info!("Kline engine created (lazy — will start on first subscribe)");

    let ob_perpetual_ws = virs_ccxt::create_orderbook_ws(config.proxy.as_deref());
    let orderbook_engine = create_orderbook_engine(
        OrderBookEngineConfig::default(),
        ob_perpetual_ws,
    );
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

    /* 优雅关闭：先停止交易引擎（平仓+取消任务），再停止行情引擎，确保资源完全释放 */
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
