//! 策略 prompt 管理 API。
//!
//! Endpoints:
//! - `POST /api/strategies/prompts/generate` — AI 生成策略 prompt
//! - `GET  /api/strategies/prompts` — 列出全部已加载的策略模板
//! - `GET  /api/strategies/prompts/{type}/{name}` — 获取指定模板
//! - `POST /api/strategies/prompts` — 保存策略模板到文件
//! - `DELETE /api/strategies/prompts/{type}/{name}` — 删除策略模板文件

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use virs_strategy::prompt::{
    delete_template, generate_prompt, save_template, GenerateRequest, PromptTemplate,
    StrategyType,
};
use virs_error::VirsError;

use crate::handlers::response::{extract_user_id, ApiResponse};
use crate::state::AppState;

/// AI 生成策略 prompt。
///
/// 请求体：
/// ```json
/// {
///   "strategy_type": "auto",
///   "user_intent": "做一个趋势跟随策略，4h定方向，1h入场",
///   "name_hint": "trend_following"  // 可选
/// }
/// ```
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

/// 列出全部已加载的策略模板。
pub async fn list(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<ApiResponse>, VirsError> {
    let _user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let loader = state.prompt_loader.clone();
    let auto_list = loader.list(StrategyType::Auto).await;

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "auto": auto_list,
    }))))
}

/// 获取指定策略模板。
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

/// 保存策略模板到文件。
///
/// 请求体：完整的 PromptTemplate JSON + `overwrite` 选项
///
/// 写入文件成功后同步更新内存缓存,使后续 `get` / `list` 立即返回新内容。
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

    let path = save_template(&tpl, overwrite)?;
    state.prompt_loader.upsert(tpl.clone()).await;

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "path": path.display().to_string(),
        "name": tpl.name,
        "strategy_type": tpl.strategy_type,
    }))))
}

/// 删除策略模板文件。
///
/// 删除文件成功后同步从内存缓存移除,使后续 `get` / `list` 不再返回已删除的策略。
pub async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((strategy_type, name)): Path<(String, String)>,
) -> Result<Json<ApiResponse>, VirsError> {
    let _user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let st = parse_strategy_type(&strategy_type)?;
    delete_template(st, &name)?;
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
