pub mod auth;
pub mod market;
pub mod strategy;
pub mod backtest;
pub mod health;
pub mod user;
pub mod middleware;
pub mod credentials;
pub mod dashboard;
pub mod plugins;
pub mod ai;

use axum::{
    routing::{get, post, put, delete},
    Router,
};
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};

use crate::config::AppConfig;
use crate::engine::StrategyEngine;
use crate::engine::plugin::PluginRegistry;

pub fn build_router(
    config: Arc<AppConfig>,
    strategy_engine: Arc<StrategyEngine>,
    db_pool: sqlx::PgPool,
    plugin_registry: Arc<PluginRegistry>,
) -> Router {
    let state = Arc::new(AppState {
        config,
        strategy_engine,
        db_pool,
        plugin_registry,
    });

    let frontend_dir = std::env::var("FRONTEND_DIR")
        .unwrap_or_else(|_| "./frontend/dist".to_string());

    Router::new()
        .route("/api/health", get(health::health_check))
        .route("/api/user/login", post(auth::login))
        .route("/api/user/logout", post(auth::logout))
        .route("/api/user/info", get(auth::get_user_info))
        .route("/api/users/list", get(user::list_users))
        .route("/api/users/create", post(user::create_user))
        .route("/api/users/update", put(user::update_user))
        .route("/api/users/delete", delete(user::delete_user))
        .route("/api/market/ticker", get(market::get_ticker))
        .route("/api/market/klines", get(market::get_klines))
        .route("/api/market/orderbook", get(market::get_order_book))
        .route("/api/market/balances", get(market::get_balances))
        .route("/api/market/symbols", get(market::get_symbols))
        .route("/api/strategies", get(strategy::list_strategies))
        .route("/api/strategies/create", post(strategy::create_strategy))
        .route("/api/strategies/{id}", get(strategy::get_strategy))
        .route("/api/strategies/{id}/update", put(strategy::update_strategy))
        .route("/api/strategies/{id}/delete", delete(strategy::delete_strategy))
        .route("/api/strategies/{id}/start", post(strategy::start_strategy))
        .route("/api/strategies/{id}/stop", post(strategy::stop_strategy))
        .route("/api/strategy/validate-script", post(strategy::validate_script))
        .route("/api/backtest/run", post(backtest::run_backtest))
        .route("/api/backtest/{id}", get(backtest::get_backtest_result))
        .route("/api/backtest/list", get(backtest::list_backtest_results))
        .route("/api/credentials/list", get(credentials::list_credentials))
        .route("/api/credentials/save", post(credentials::save_credential))
        .route("/api/credentials/delete/{id}", delete(credentials::delete_credential))
        .route("/api/credentials/test", post(credentials::test_credential))
        .route("/api/dashboard/summary", get(dashboard::dashboard_summary))
        .route("/api/positions", get(dashboard::list_positions))
        .route("/api/trades", get(dashboard::list_trades))
        .route("/api/pending-orders", get(dashboard::list_pending_orders))
        .route("/api/plugins", get(plugins::list_plugins))
        .route("/api/ai/status", get(ai::ai_status))
        .route("/api/ai/generate", post(ai::generate_strategy))
        .with_state(state)
        .nest_service("/", ServeDir::new(&frontend_dir)
            .fallback(ServeFile::new(format!("{}/index.html", frontend_dir))))
}

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub strategy_engine: Arc<StrategyEngine>,
    pub db_pool: sqlx::PgPool,
    pub plugin_registry: Arc<PluginRegistry>,
}
