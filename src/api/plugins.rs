use axum::{extract::State, Json};
use std::sync::Arc;

use crate::api::middleware::AuthUser;
use crate::api::AppState;
use crate::models::*;

pub async fn list_plugins(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
) -> Json<ApiResponse<Vec<serde_json::Value>>> {
    let plugins = state.plugin_registry.list();
    let data: Vec<serde_json::Value> = plugins
        .iter()
        .map(|p| {
            serde_json::json!({
                "name": p.name,
                "description": p.description,
                "category": p.category,
                "params": p.params,
            })
        })
        .collect();
    Json(ApiResponse::ok(data))
}
