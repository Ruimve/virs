use axum::{
    http::{header, StatusCode, Uri},
    response::{Html, IntoResponse, Response},
    routing::{delete, get, post, put},
    Router,
};
use tower_http::cors::{Any, CorsLayer};

use crate::handlers;
use crate::state::AppState;


pub fn build_router(state: AppState) -> Router {
    Router::new()

        .route("/api/health", get(handlers::health::health_check))

        .route("/api/user/login", post(handlers::auth::login))
        .route("/api/user/logout", post(handlers::auth::logout))
        .route("/api/user/info", get(handlers::auth::get_user_info))

        .route("/api/users/list", get(handlers::user::list_users))
        .route("/api/users/create", post(handlers::user::create_user))
        .route("/api/users/update", put(handlers::user::update_user))
        .route("/api/users/delete", delete(handlers::user::delete_user))

        .route("/api/market/ticker", get(handlers::market::get_ticker))
        .route("/api/market/klines", get(handlers::market::get_klines))
        .route(
            "/api/market/orderbook",
            get(handlers::market::get_order_book),
        )
        .route("/api/market/balances", get(handlers::market::get_balances))
        .route("/api/market/symbols", get(handlers::market::get_symbols))

        .route(
            "/api/kline/subscribe",
            post(handlers::market::kline_subscribe),
        )
        .route("/api/kline/data", get(handlers::market::kline_data))

        .route(
            "/api/orderbook/subscribe",
            post(handlers::market::orderbook_subscribe),
        )

        .route(
            "/api/credentials/list",
            get(handlers::credentials::list_credentials),
        )
        .route(
            "/api/credentials/save",
            post(handlers::credentials::save_credential),
        )
        .route(
            "/api/credentials/delete/{id}",
            delete(handlers::credentials::delete_credential),
        )
        .route(
            "/api/credentials/test",
            get(handlers::credentials::test_credential),
        )
        .route(
            "/api/credentials/check-permissions",
            get(handlers::credentials::check_permissions),
        )
        .route(
            "/api/credentials/verify",
            post(handlers::credentials::verify_permissions),
        )
        .route(
            "/api/credentials/position-mode",
            get(handlers::credentials::check_position_mode),
        )
        .route(
            "/api/credentials/status",
            get(handlers::credentials::exchange_status),
        )

        .route("/api/ai/status", get(handlers::ai::ai_status))
        .route("/api/ai/optimize", post(handlers::ai::optimize))
        .route("/api/ai/explain", post(handlers::ai::explain))
        .route(
            "/api/ai/recommend-strategy",
            post(handlers::ai::recommend_strategy),
        )

        .route(
            "/api/ai-credentials/list",
            get(handlers::ai_credentials::list_credentials),
        )
        .route(
            "/api/ai-credentials/save",
            post(handlers::ai_credentials::save_credential),
        )
        .route(
            "/api/ai-credentials/delete/{id}",
            delete(handlers::ai_credentials::delete_credential),
        )
        .route(
            "/api/ai-credentials/test",
            get(handlers::ai_credentials::test_credential),
        )
        .route(
            "/api/ai-credentials/models",
            get(handlers::ai_credentials::fetch_models),
        )
        .route(
            "/api/ai-credentials/balance",
            get(handlers::ai_credentials::fetch_balance),
        )

        .route("/api/bot/create", post(handlers::bot_trade::create_bot))
        .route("/api/bot/list", get(handlers::bot_trade::list_bots))
        .route(
            "/api/bot/{id}/analysis-logs",
            get(handlers::bot_trade::get_analysis_logs),
        )
        .route("/api/bot/{id}", get(handlers::bot_trade::get_bot))
        .route("/api/bot/{id}", put(handlers::bot_trade::update_bot))
        .route(
            "/api/bot/{id}/start",
            post(handlers::bot_trade::start_bot),
        )
        .route("/api/bot/{id}/stop", post(handlers::bot_trade::stop_bot))
        .route(
            "/api/bot/{id}/delete",
            delete(handlers::bot_trade::delete_bot),
        )
        .route(
            "/api/bot/{id}/trades",
            get(handlers::bot_trade::get_trades),
        )
        .route("/api/bot/{id}/stats", get(handlers::bot_trade::get_stats))


        .route(
            "/api/strategies/prompts/generate",
            post(handlers::strategy::generate),
        )
        .route(
            "/api/strategies/prompts",
            get(handlers::strategy::list).post(handlers::strategy::save),
        )
        .route(
            "/api/strategies/prompts/{strategy_type}/{name}",
            get(handlers::strategy::get).delete(handlers::strategy::delete),
        )

        .route(
            "/api/system/paper/status",
            get(handlers::system::paper_status),
        )
        .route("/api/system/info", get(handlers::system::system_info))

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

/* SPA fallback：未匹配API路由的请求返回前端静态文件，支持前端单页应用的路由 */
async fn spa_fallback(uri: Uri) -> Response {
    let frontend_dir =
        std::env::var("FRONTEND_DIR").unwrap_or_else(|_| "./apps/web/dist".to_string());

    let path = uri.path().trim_start_matches('/');

    if path.is_empty() || path == "index.html" {
        return serve_index_html(&frontend_dir);
    }

    let candidate = std::path::Path::new(&frontend_dir).join(path);

    if candidate.is_file() && candidate.starts_with(&frontend_dir) {
        match std::fs::read(&candidate) {
            Ok(content) => {
                let mime = mime_guess_from_ext(
                    candidate.extension().and_then(|e| e.to_str()).unwrap_or(""),
                );
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
