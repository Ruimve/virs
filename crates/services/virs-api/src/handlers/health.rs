//! Health check handler.

use axum::Json;

use crate::handlers::response::ApiResponse;

pub async fn health_check() -> Json<ApiResponse> {
    Json(ApiResponse::ok(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    })))
}
