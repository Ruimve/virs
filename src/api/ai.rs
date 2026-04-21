use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
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

#[derive(Deserialize)]
pub struct OptimizeRequest {
    pub strategy_code: Option<String>,
    pub backtest_summary: serde_json::Value,
    pub provider: Option<String>,
    pub model: Option<String>,
}

#[derive(Deserialize)]
pub struct ExplainRequest {
    pub strategy_code: String,
    pub provider: Option<String>,
    pub model: Option<String>,
}

/// AI parameter optimization suggestions based on backtest results.
pub async fn optimize(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
    Json(body): Json<OptimizeRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)>
{
    let ai_service = AiService::new(state.config.ai.clone());

    if !ai_service.is_configured() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<serde_json::Value>::err(
                "AI service is not configured",
            )),
        ));
    }

    match ai_service
        .optimize_strategy(
            body.strategy_code.as_deref(),
            &body.backtest_summary,
            body.provider.as_deref(),
            body.model.as_deref(),
        )
        .await
    {
        Ok(suggestion) => Ok(Json(ApiResponse::ok(serde_json::json!({
            "suggestion": suggestion,
        })))),
        Err(e) => {
            tracing::error!("AI optimization failed: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<serde_json::Value>::err(format!(
                    "AI optimization failed: {}",
                    e
                ))),
            ))
        }
    }
}

/// AI strategy explanation.
pub async fn explain(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
    Json(body): Json<ExplainRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)>
{
    let ai_service = AiService::new(state.config.ai.clone());

    if !ai_service.is_configured() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<serde_json::Value>::err(
                "AI service is not configured",
            )),
        ));
    }

    match ai_service
        .explain_strategy(
            &body.strategy_code,
            body.provider.as_deref(),
            body.model.as_deref(),
        )
        .await
    {
        Ok(explanation) => Ok(Json(ApiResponse::ok(serde_json::json!({
            "explanation": explanation,
        })))),
        Err(e) => {
            tracing::error!("AI explanation failed: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<serde_json::Value>::err(format!(
                    "AI explanation failed: {}",
                    e
                ))),
            ))
        }
    }
}
