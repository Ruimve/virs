//! System-level handlers — paper mode, engine status.

use axum::{
    extract::State,
    http::HeaderMap,
    Json,
};

use crate::handlers::response::{extract_user_id, ApiResponse};
use crate::state::AppState;

pub async fn paper_status(
    State(state): State<AppState>,
) -> Json<ApiResponse> {
    Json(ApiResponse::ok(serde_json::json!({
        "paper_mode": state.engine_manager.paper_mode(),
    })))
}

pub async fn paper_enable(
    State(state): State<AppState>,
    _headers: HeaderMap,
) -> Result<Json<ApiResponse>, (axum::http::StatusCode, Json<ApiResponse>)> {
    let _user_id = extract_user_id(&_headers)?;
    state.ws_broadcaster.broadcast(serde_json::json!({
        "type": "paper_mode",
        "enabled": true,
    }));
    Ok(Json(ApiResponse::ok(serde_json::json!({
        "paper_mode": true,
        "message": "Paper mode is configured at startup. Restart the server with PAPER_MODE=true to enable.",
    }))))
}

pub async fn paper_disable(
    State(state): State<AppState>,
    _headers: HeaderMap,
) -> Result<Json<ApiResponse>, (axum::http::StatusCode, Json<ApiResponse>)> {
    let _user_id = extract_user_id(&_headers)?;
    state.ws_broadcaster.broadcast(serde_json::json!({
        "type": "paper_mode",
        "enabled": false,
    }));
    Ok(Json(ApiResponse::ok(serde_json::json!({
        "paper_mode": false,
        "message": "Paper mode is configured at startup. Restart the server with PAPER_MODE=false to disable.",
    }))))
}
