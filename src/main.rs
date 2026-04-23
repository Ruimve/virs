use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod api;
mod ccxt;
mod config;
mod engine;
mod exchange;
mod models;
mod order_worker;
mod services;
mod utils;

use config::load_config;
use engine::{StrategyEngine, StrategyEngineConfig};
use exchange::Exchange;

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

    // Create order channel
    let (order_tx, mut order_rx) = mpsc::channel::<engine::OrderCommand>(1000);

    // Create plugin registry and register built-in plugins
    let mut plugin_registry = engine::plugin::PluginRegistry::new();
    plugin_registry.register(Box::new(engine::plugins::SmaCrossoverPlugin));
    plugin_registry.register(Box::new(engine::plugins::RsiPlugin));
    plugin_registry.register(Box::new(engine::plugins::MacdPlugin));
    plugin_registry.register(Box::new(engine::plugins::BollingerBandsPlugin));
    let plugin_registry = Arc::new(plugin_registry);
    info!(
        "Registered {} indicator plugins",
        plugin_registry.list().len()
    );

    // Create strategy engine
    let strategy_engine_config = StrategyEngineConfig {
        executor_workers: config.strategy.executor_workers,
        pending_order_poll_interval_secs: config.strategy.pending_order_poll_interval_secs,
        auto_restore: config.strategy.auto_restore_strategies,
    };
    let mut strategy_engine_inner = StrategyEngine::new(strategy_engine_config, order_tx, plugin_registry.clone());

    // Create WebSocket broadcaster for real-time push
    let ws_broadcaster = Arc::new(api::ws::create_broadcaster(256));
    strategy_engine_inner.set_ws_broadcaster(ws_broadcaster.clone());
    strategy_engine_inner.set_db_pool(db_pool.clone());
    let strategy_engine = Arc::new(strategy_engine_inner);

    // Exchange credentials are now loaded from the database (qd_exchange_credentials)
    // on demand when a strategy is started or market data is requested.
    // No exchange instances are registered at startup.
    info!("ℹ️  Exchange credentials loaded on-demand from database (Credentials page)");

    // Spawn order worker — executes real trades via exchange
    let worker = order_worker::OrderWorker::new(
        db_pool.clone(),
        strategy_engine.clone(),
        config.notification.clone(),
        ws_broadcaster.clone(),
    );
    tokio::spawn(async move {
        worker.run(order_rx).await;
    });

    // Spawn pending order retry worker
    if config.strategy.pending_order_worker_enabled {
        let retry_pool = db_pool.clone();
        let retry_engine = strategy_engine.clone();
        let retry_interval = config.strategy.pending_order_poll_interval_secs;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(retry_interval));
            loop {
                interval.tick().await;

                // Find failed orders that can be retried
                #[derive(sqlx::FromRow)]
                struct RetryOrderRow {
                    id: uuid::Uuid,
                    strategy_id: uuid::Uuid,
                    symbol: String,
                    order_type: String,
                    side: String,
                    amount: f64,
                    price: Option<f64>,
                    attempts: i32,
                    max_attempts: i32,
                }

                let orders = sqlx::query_as::<_, RetryOrderRow>(
                    r#"SELECT id, strategy_id, symbol, order_type, side,
                       amount, price, attempts, max_attempts
                       FROM pending_orders
                       WHERE status = 'failed' AND attempts < max_attempts
                       ORDER BY created_at ASC LIMIT 10"#
                )
                .fetch_all(&retry_pool)
                .await
                .unwrap_or_default();

                for order in orders {
                    info!("Retrying pending order {} (attempt {}/{})", order.id, order.attempts + 1, order.max_attempts);

                    // Get exchange name from strategy
                    let strategy: Option<(String,)> = sqlx::query_as(
                        "SELECT exchange FROM qd_strategies_trading WHERE id = $1"
                    )
                    .bind(order.strategy_id)
                    .fetch_optional(&retry_pool)
                    .await
                    .unwrap_or(None);

                    if let Some((exchange_name,)) = strategy {
                        if let Some(exchange) = retry_engine.get_exchange(&exchange_name) {
                            let side_str = &order.side;
                            let side = if side_str == "buy" { models::Side::Buy } else { models::Side::Sell };
                            let order_type = match order.order_type.as_str() {
                                "market" => models::OrderType::Market,
                                "limit" => models::OrderType::Limit,
                                "stop_market" => models::OrderType::StopMarket,
                                "stop_limit" => models::OrderType::StopLimit,
                                _ => models::OrderType::Market,
                            };

                            match exchange.place_order(&order.symbol, side, order_type, order.amount, order.price).await {
                                Ok(o) => {
                                    let _ = sqlx::query("UPDATE pending_orders SET status = 'filled', exchange_order_id = $1, updated_at = NOW() WHERE id = $2")
                                        .bind(&o.id)
                                        .bind(order.id)
                                        .execute(&retry_pool)
                                        .await;
                                    info!("✅ Retry succeeded for order {}", order.id);
                                }
                                Err(e) => {
                                    let _ = sqlx::query("UPDATE pending_orders SET attempts = attempts + 1, error_message = $1, status = CASE WHEN attempts + 1 >= max_attempts THEN 'failed' ELSE 'pending' END, updated_at = NOW() WHERE id = $2")
                                        .bind(e.to_string())
                                        .bind(order.id)
                                        .execute(&retry_pool)
                                        .await;
                                }
                            }
                        }
                    }
                }
            }
        });
        info!("✅ Pending order retry worker started (interval: {}s)", retry_interval);
    }

    // Build and start HTTP server
    let app = api::build_router(Arc::new(config.clone()), strategy_engine, db_pool, plugin_registry, ws_broadcaster);

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
