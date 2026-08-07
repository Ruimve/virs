

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use virs_prompt::{create_strategy, delete_strategy, PromptTemplate, ENV_STRATEGIES_DIR};
use virs_tactical_bot::{generate_prompt, GenerateRequest};
use virs_type::StrategyType;
use virs_error::VirsError;

use crate::handlers::response::{extract_user_id, ApiResponse};
use crate::state::AppState;


pub async fn generate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiResponse>, VirsError> {
    let _user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let strategy_type = body["strategy_type"]
        .as_str()
        .ok_or_else(|| VirsError::bad_request("strategy_type is required (auto)"))?;
    let strategy_type = match strategy_type {
        "auto" => StrategyType::Auto,
        _ => return Err(VirsError::bad_request("strategy_type must be 'auto'")),
    };

    let user_intent = body["user_intent"]
        .as_str()
        .ok_or_else(|| VirsError::bad_request("user_intent is required"))?;
    if user_intent.trim().is_empty() {
        return Err(VirsError::bad_request("user_intent must not be empty"));
    }

    let name_hint = body["name_hint"].as_str().filter(|s| !s.is_empty());

    let (api_key, base_url, model) = state.resolve_llm_credentials().await?;

    let result = generate_prompt(
        &state.http_client,
        &api_key,
        &base_url,
        &model,
        GenerateRequest {
            strategy_type,
            user_intent,
            name_hint,
        },
    )
    .await?;

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "template": result.template,
        "used_model": result.used_model,
    }))))
}


pub async fn list(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<ApiResponse>, VirsError> {
    let _user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let loader = state.prompt_loader.clone();
    let auto_list = loader.list(StrategyType::Auto).await;

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "auto": auto_list,
    }))))
}


pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((strategy_type, name)): Path<(String, String)>,
) -> Result<Json<ApiResponse>, VirsError> {
    let _user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let st = parse_strategy_type(&strategy_type)?;
    let loader = state.prompt_loader.clone();
    let tpl = loader
        .get(st, &name)
        .await
        .ok_or_else(|| VirsError::not_found(format!("策略模板不存在: {strategy_type}/{name}")))?;

    Ok(Json(ApiResponse::ok(serde_json::json!(tpl))))
}


pub async fn save(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiResponse>, VirsError> {
    let _user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let overwrite = body["overwrite"].as_bool().unwrap_or(false);
    let tpl: PromptTemplate = serde_json::from_value(body["template"].clone()).map_err(|e| {
        VirsError::bad_request(format!("Invalid PromptTemplate JSON: {e}"))
    })?;

    if overwrite {
        if let Ok(dir) = std::env::var(ENV_STRATEGIES_DIR) {
            if std::path::PathBuf::from(&dir).join(&tpl.name).exists() {
                delete_strategy(&tpl.name)?;
            }
        }
    }

    let path = create_strategy(&tpl)?;
    state.prompt_loader.upsert(tpl.clone()).await;

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "path": path.display().to_string(),
        "name": tpl.name,
        "strategy_type": tpl.strategy_type,
    }))))
}


pub async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((strategy_type, name)): Path<(String, String)>,
) -> Result<Json<ApiResponse>, VirsError> {
    let _user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let st = parse_strategy_type(&strategy_type)?;
    delete_strategy(&name)?;
    state.prompt_loader.remove(st, &name).await;

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "deleted": format!("{strategy_type}/{name}")
    }))))
}

fn parse_strategy_type(s: &str) -> Result<StrategyType, VirsError> {
    match s {
        "auto" => Ok(StrategyType::Auto),
        _ => Err(VirsError::bad_request("strategy_type must be 'auto'")),
    }
}
