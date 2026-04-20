use axum::{extract::State, http::StatusCode, Json};
use std::sync::Arc;

use crate::api::middleware::AuthUser;
use crate::api::AppState;
use crate::models::*;
use crate::services::ai::{AiService, GenerateRequest};

/// Check AI service availability and list providers.
pub async fn ai_status(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
) -> Json<ApiResponse<serde_json::Value>> {
    let ai_service = AiService::new(state.config.ai.clone());
    Json(ApiResponse::ok(serde_json::json!({
        "configured": ai_service.is_configured(),
        "providers": ai_service.available_providers(),
    })))
}

/// Generate a Lua strategy from natural language.
pub async fn generate_strategy(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
    Json(body): Json<GenerateRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)>
{
    if body.prompt.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<serde_json::Value>::err("Prompt cannot be empty")),
        ));
    }

    let ai_service = AiService::new(state.config.ai.clone());

    if !ai_service.is_configured() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::<serde_json::Value>::err(
                "No AI provider configured. Set OPENROUTER_API_KEY, OPENAI_API_KEY, or DEEPSEEK_API_KEY in .env",
            )),
        ));
    }

    match ai_service.generate_strategy(&body).await {
        Ok(response) => Ok(Json(ApiResponse::ok(
            serde_json::to_value(&response).unwrap_or_default(),
        ))),
        Err(e) => {
            tracing::error!("AI strategy generation failed: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<serde_json::Value>::err(format!(
                    "AI generation failed: {}",
                    e
                ))),
            ))
        }
    }
}
