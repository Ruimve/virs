//! Health check handler.

use axum::Json;
use virs_error::VirsError;

use crate::handlers::response::ApiResponse;

pub async fn health_check() -> Result<Json<ApiResponse>, VirsError> {
    Ok(Json(ApiResponse::ok(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))))
}
