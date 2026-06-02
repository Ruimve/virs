pub mod auth;
pub mod market;
pub mod health;
pub mod user;
pub mod middleware;
pub mod credentials;
pub mod dashboard;
pub mod ai;
pub mod ai_credentials;
pub mod ws;
pub mod grid;
pub mod kline;
pub mod auto_trade;

use axum::{
    http::{header, StatusCode, Uri},
    response::{Html, IntoResponse, Response},
    routing::{get, post, put, delete},
    Router,
};
use tower_http::cors::{CorsLayer, Any};
use std::sync::Arc;

use crate::config::AppConfig;
use crate::trading::exchange::registry::ExchangeRegistry;

pub fn build_router(
    config: Arc<AppConfig>,
    exchange_registry: Arc<ExchangeRegistry>,
    db_pool: sqlx::PgPool,
    ws_broadcaster: Arc<ws::WsBroadcaster>,
    kline_engine: Option<Arc<crate::engine::kline::KlineEngine>>,
    grid_cmd_tx: Option<tokio::sync::mpsc::Sender<crate::bot::semi_automatic_grid::types::GridCommand>>,
    auto_cmd_tx: Option<tokio::sync::mpsc::Sender<crate::bot::auto_trade::types::AutoCommand>>,
    paper_executor: Option<Arc<crate::trading::paper::PaperOrderExecutor>>,
) -> Router {
    let state = Arc::new(AppState {
        config,
        exchange_registry,
        db_pool,
        ws_broadcaster,
        kline_engine,
        http_client: reqwest::Client::new(),
        grid_cmd_tx,
        auto_cmd_tx,
        paper_executor,
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
        .route("/api/credentials/list", get(credentials::list_credentials))
        .route("/api/credentials/save", post(credentials::save_credential))
        .route("/api/credentials/delete/{id}", delete(credentials::delete_credential))
        .route("/api/credentials/test", post(credentials::test_credential))
        .route("/api/dashboard/summary", get(dashboard::dashboard_summary))
        .route("/api/positions", get(dashboard::list_positions))
        .route("/api/trades", get(dashboard::list_trades))
        .route("/api/pending-orders", get(dashboard::list_pending_orders))
        .route("/api/ai/status", get(ai::ai_status))
        .route("/api/ai/optimize", post(ai::optimize))
        .route("/api/ai/explain", post(ai::explain))
        .route("/api/ai/recommend-strategy", post(ai::recommend_strategy))
        .route("/api/ai-credentials/list", get(ai_credentials::list_credentials))
        .route("/api/ai-credentials/save", post(ai_credentials::save_credential))
        .route("/api/ai-credentials/delete/{id}", delete(ai_credentials::delete_credential))
        .route("/api/ai-credentials/test", post(ai_credentials::test_credential))
        .route("/api/kline/data", get(crate::api::kline::get_klines))
        .route("/api/kline/subscribe", post(crate::api::kline::subscribe_kline))
        .route("/api/kline/unsubscribe", post(crate::api::kline::unsubscribe_kline))
        .route("/api/kline/subscriptions", get(crate::api::kline::list_subscriptions))
        .route("/api/kline/backtest/limits", get(crate::api::kline::get_backtest_limits))
        .route("/api/kline/backtest/data", get(crate::api::kline::get_backtest_data))
        .route("/ws/kline", get(crate::api::kline::kline_ws_handler))
        .route("/ws", get(ws::ws_handler))
        .route("/api/grid/create", post(grid::create_bot))
        .route("/api/grid/list", get(grid::list_bots))
        .route("/api/grid/analysis-logs", get(grid::get_analysis_logs))
        .route("/api/grid/paper/status", get(grid::paper_status))
        .route("/api/grid/paper/enable", post(grid::paper_enable))
        .route("/api/grid/paper/disable", post(grid::paper_disable))
        .route("/api/grid/{id}", get(grid::get_bot))
        .route("/api/grid/{id}/start", post(grid::start_bot))
        .route("/api/grid/{id}/stop", post(grid::stop_bot))
        .route("/api/grid/{id}/delete", delete(grid::delete_bot))
        .route("/api/grid/{id}/trades", get(grid::get_trades))
        .route("/api/auto/create", post(auto_trade::create_bot))
        .route("/api/auto/list", get(auto_trade::list_bots))
        .route("/api/auto/analysis-logs", get(auto_trade::get_analysis_logs))
        .route("/api/auto/{id}", get(auto_trade::get_bot))
        .route("/api/auto/{id}/start", post(auto_trade::start_bot))
        .route("/api/auto/{id}/stop", post(auto_trade::stop_bot))
        .route("/api/auto/{id}/delete", delete(auto_trade::delete_bot))
        .route("/api/auto/{id}/trades", get(auto_trade::get_trades))
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

pub fn normalize_symbol(raw: &str) -> String {
    let s = raw.trim().to_uppercase();
    if s.contains('/') {
        return s;
    }
    let quotes = [
        "USDT", "USDC", "BUSD", "BTC", "ETH", "BNB", "EUR", "GBP", "TRY", "BRL", "ARS",
    ];
    for q in &quotes {
        if s.ends_with(q) {
            let base = &s[..s.len() - q.len()];
            if !base.is_empty() {
                return format!("{}/{}", base, q);
            }
        }
    }
    s
}

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub exchange_registry: Arc<ExchangeRegistry>,
    pub db_pool: sqlx::PgPool,
    pub ws_broadcaster: Arc<ws::WsBroadcaster>,
    pub kline_engine: Option<Arc<crate::engine::kline::KlineEngine>>,
    pub http_client: reqwest::Client,
    pub grid_cmd_tx: Option<tokio::sync::mpsc::Sender<crate::bot::semi_automatic_grid::types::GridCommand>>,
    pub auto_cmd_tx: Option<tokio::sync::mpsc::Sender<crate::bot::auto_trade::types::AutoCommand>>,
    pub paper_executor: Option<Arc<crate::trading::paper::PaperOrderExecutor>>,
}
