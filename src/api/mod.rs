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
pub mod ws;

use axum::{
    http::{header, StatusCode, Uri},
    response::{Html, IntoResponse, Response},
    routing::{get, post, put, delete},
    Router,
};
use tower_http::cors::{CorsLayer, Any};
use std::sync::Arc;

use crate::config::AppConfig;
use crate::engine::StrategyEngine;
use crate::engine::plugin::PluginRegistry;

pub fn build_router(
    config: Arc<AppConfig>,
    strategy_engine: Arc<StrategyEngine>,
    db_pool: sqlx::PgPool,
    plugin_registry: Arc<PluginRegistry>,
    ws_broadcaster: Arc<ws::WsBroadcaster>,
) -> Router {
    let state = Arc::new(AppState {
        config,
        strategy_engine,
        db_pool,
        plugin_registry,
        ws_broadcaster,
    });

    let _frontend_dir = std::env::var("FRONTEND_DIR")
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
        .route("/api/ai/optimize", post(ai::optimize))
        .route("/api/ai/explain", post(ai::explain))
        .route("/ws", get(ws::ws_handler))
        .with_state(state)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .fallback(spa_fallback)
}

async fn spa_fallback(uri: Uri) -> Response {
    let frontend_dir = std::env::var("FRONTEND_DIR")
        .unwrap_or_else(|_| "./frontend/dist".to_string());

    let path = uri.path().trim_start_matches('/');

    if path.is_empty() || path == "index.html" {
        return serve_index_html(&frontend_dir);
    }

    let candidate = std::path::Path::new(&frontend_dir).join(path);

    if candidate.is_file() && candidate.starts_with(&frontend_dir) {
        match std::fs::read(&candidate) {
            Ok(content) => {
                let mime = mime_guess_from_ext(candidate.extension().and_then(|e| e.to_str()).unwrap_or(""));
                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, mime)
                    .body(axum::body::Body::from(content))
                    .unwrap()
            }
            Err(_) => (StatusCode::NOT_FOUND, "File not found").into_response(),
        }
    } else {
        serve_index_html(&frontend_dir)
    }
}

fn serve_index_html(frontend_dir: &str) -> Response {
    let index_path = format!("{}/index.html", frontend_dir);
    match std::fs::read_to_string(&index_path) {
        Ok(html) => Html(html).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Frontend not built").into_response(),
    }
}

fn mime_guess_from_ext(ext: &str) -> &'static str {
    match ext {
        "js" => "application/javascript",
        "css" => "text/css",
        "html" => "text/html; charset=utf-8",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "woff" | "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "map" => "application/json",
        _ => "application/octet-stream",
    }
}

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub strategy_engine: Arc<StrategyEngine>,
    pub db_pool: sqlx::PgPool,
    pub plugin_registry: Arc<PluginRegistry>,
    pub ws_broadcaster: Arc<ws::WsBroadcaster>,
}
