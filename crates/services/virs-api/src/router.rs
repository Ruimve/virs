//! API router — route definitions and SPA fallback.

use axum::{
    http::{header, StatusCode, Uri},
    response::{Html, IntoResponse, Response},
    routing::{delete, get, post, put},
    Router,
};
use tower_http::cors::{Any, CorsLayer};

use crate::handlers;
use crate::state::AppState;

/// 构建 API 路由
pub fn build_router(state: AppState) -> Router {
    Router::new()
        // Health (public)
        .route("/api/health", get(handlers::health::health_check))
        // Auth (public)
        .route("/api/user/login", post(handlers::auth::login))
        .route("/api/user/logout", post(handlers::auth::logout))
        .route("/api/user/info", get(handlers::auth::get_user_info))
        // Users (authenticated)
        .route("/api/users/list", get(handlers::user::list_users))
        .route("/api/users/create", post(handlers::user::create_user))
        .route("/api/users/update", put(handlers::user::update_user))
        .route("/api/users/delete", delete(handlers::user::delete_user))
        // Market (public — market data is read-only)
        .route("/api/market/ticker", get(handlers::market::get_ticker))
        .route("/api/market/klines", get(handlers::market::get_klines))
        .route("/api/market/orderbook", get(handlers::market::get_order_book))
        .route("/api/market/balances", get(handlers::market::get_balances))
        .route("/api/market/symbols", get(handlers::market::get_symbols))
        // Kline Engine (public)
        .route("/api/kline/subscribe", post(handlers::market::kline_subscribe))
        .route("/api/kline/data", get(handlers::market::kline_data))
        // OrderBook Engine (public)
        .route("/api/orderbook/subscribe", post(handlers::market::orderbook_subscribe))
        // Credentials (authenticated)
        .route("/api/credentials/list", get(handlers::credentials::list_credentials))
        .route("/api/credentials/save", post(handlers::credentials::save_credential))
        .route("/api/credentials/delete/{id}", delete(handlers::credentials::delete_credential))
        .route("/api/credentials/test", get(handlers::credentials::test_credential))
        .route("/api/credentials/check-permissions", get(handlers::credentials::check_permissions))
        .route("/api/credentials/verify", post(handlers::credentials::verify_permissions))
        .route("/api/credentials/status", get(handlers::credentials::exchange_status))
        // Dashboard (authenticated)
        .route("/api/dashboard/summary", get(handlers::dashboard::dashboard_summary))
        .route("/api/positions", get(handlers::dashboard::list_positions))
        .route("/api/trades", get(handlers::dashboard::list_trades))
        .route("/api/pending-orders", get(handlers::dashboard::list_pending_orders))
        // AI (authenticated)
        .route("/api/ai/status", get(handlers::ai::ai_status))
        .route("/api/ai/optimize", post(handlers::ai::optimize))
        .route("/api/ai/explain", post(handlers::ai::explain))
        .route("/api/ai/recommend-strategy", post(handlers::ai::recommend_strategy))
        // AI Credentials (authenticated)
        .route("/api/ai-credentials/list", get(handlers::ai_credentials::list_credentials))
        .route("/api/ai-credentials/save", post(handlers::ai_credentials::save_credential))
        .route("/api/ai-credentials/delete/{id}", delete(handlers::ai_credentials::delete_credential))
        .route("/api/ai-credentials/test", get(handlers::ai_credentials::test_credential))
        .route("/api/ai-credentials/models", get(handlers::ai_credentials::fetch_models))
        .route("/api/ai-credentials/balance", get(handlers::ai_credentials::fetch_balance))
        // Grid Bot (authenticated)
        .route("/api/grid/create", post(handlers::grid::create_bot))
        .route("/api/grid/list", get(handlers::grid::list_bots))
        .route("/api/grid/analysis-logs", get(handlers::grid::get_analysis_logs))
        .route("/api/grid/{id}", get(handlers::grid::get_bot))
        .route("/api/grid/{id}/start", post(handlers::grid::start_bot))
        .route("/api/grid/{id}/stop", post(handlers::grid::stop_bot))
        .route("/api/grid/{id}/delete", delete(handlers::grid::delete_bot))
        .route("/api/grid/{id}/trades", get(handlers::grid::get_trades))
        .route("/api/grid/{id}/stats", get(handlers::grid::get_stats))
        // Auto Bot (authenticated)
        .route("/api/auto/create", post(handlers::auto_trade::create_bot))
        .route("/api/auto/list", get(handlers::auto_trade::list_bots))
        .route("/api/auto/analysis-logs", get(handlers::auto_trade::get_analysis_logs))
        .route("/api/auto/{id}", get(handlers::auto_trade::get_bot))
        .route("/api/auto/{id}/start", post(handlers::auto_trade::start_bot))
        .route("/api/auto/{id}/stop", post(handlers::auto_trade::stop_bot))
        .route("/api/auto/{id}/delete", delete(handlers::auto_trade::delete_bot))
        .route("/api/auto/{id}/trades", get(handlers::auto_trade::get_trades))
        .route("/api/auto/{id}/stats", get(handlers::auto_trade::get_stats))
        // System (authenticated)
        .route("/api/system/paper/status", get(handlers::system::paper_status))
        .route("/api/system/paper/enable", post(handlers::system::paper_enable))
        .route("/api/system/paper/disable", post(handlers::system::paper_disable))
        .route("/api/system/info", get(handlers::system::system_info))
        // WebSocket (public)
        .route("/ws", get(crate::ws::ws_handler))
        .route("/ws/kline", get(crate::ws::kline_ws_handler))
        .route("/ws/orderbook", get(crate::ws::orderbook_ws_handler))
        .route("/ws/position", get(crate::ws::position_ws_handler))
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

/// Normalize raw symbol string to BASE/QUOTE format
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
