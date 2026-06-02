use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::api::middleware::AuthUser;
use crate::models::{ApiResponse, MarketType};
use crate::api::AppState;

fn get_user_id(auth: &AuthUser) -> Uuid {
    auth.uuid().unwrap_or_else(|_| Uuid::nil())
}

#[derive(Debug, Deserialize)]
pub struct CreateBotRequest {
    pub name: String,
    pub symbol: String,
    #[serde(default = "default_exchange")]
    pub exchange: String,
    #[serde(default = "default_market_type")]
    pub market_type: String,
    #[serde(default = "default_leverage")]
    pub leverage: i32,
    #[serde(default = "default_max_position_pct")]
    pub max_position_pct: f64,
    #[serde(default = "default_decide_interval")]
    pub decide_interval_secs: i32,
    pub system_prompt: Option<String>,
}

fn default_exchange() -> String { "binance".to_string() }
fn default_market_type() -> String { "perpetual".to_string() }
fn default_leverage() -> i32 { 3 }
fn default_max_position_pct() -> f64 { 80.0 }
fn default_decide_interval() -> i32 { 300 }

pub async fn create_bot(
    State(state): State<std::sync::Arc<AppState>>,
    auth: AuthUser,
    Json(body): Json<CreateBotRequest>,
) -> Json<ApiResponse<serde_json::Value>> {
    let user_id = get_user_id(&auth);

    let symbol = crate::api::normalize_symbol(&body.symbol);

    let market_type = body.market_type.to_lowercase();
    if market_type != "perpetual" && market_type != "spot" {
        return Json(ApiResponse::err("market_type must be 'perpetual' or 'spot'"));
    }
    if market_type == "spot" && body.leverage > 1 {
        return Json(ApiResponse::err("spot market does not support leverage > 1"));
    }

    let bot_id = Uuid::new_v4();
    let result = sqlx::query(
        r#"INSERT INTO qd_auto_bots (
            id, user_id, name, symbol, exchange, market_type, status,
            leverage, max_position_pct, decide_interval_secs,
            system_prompt
        ) VALUES ($1, $2, $3, $4, $5, $6, 'draft', $7, $8, $9, $10)"#,
    )
    .bind(bot_id)
    .bind(user_id)
    .bind(&body.name)
    .bind(&symbol)
    .bind(&body.exchange)
    .bind(&market_type)
    .bind(body.leverage)
    .bind(body.max_position_pct)
    .bind(body.decide_interval_secs)
    .bind(&body.system_prompt)
    .execute(&state.db_pool)
    .await;

    match result {
        Ok(_) => Json(ApiResponse::ok(serde_json::json!({ "id": bot_id }))),
        Err(e) => Json(ApiResponse::err(format!("Failed to create auto bot: {}", e))),
    }
}

#[derive(Debug, Deserialize)]
pub struct ListBotsQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

pub async fn list_bots(
    State(state): State<std::sync::Arc<AppState>>,
    auth: AuthUser,
    Query(query): Query<ListBotsQuery>,
) -> Json<ApiResponse<serde_json::Value>> {
    let user_id = get_user_id(&auth);

    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).min(100);
    let offset = (page - 1) * page_size;

    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM qd_auto_bots WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(&state.db_pool)
    .await
    .unwrap_or((0,));

    let bots: Vec<crate::bot::auto_trade::types::AutoBot> = sqlx::query_as(
        r#"SELECT * FROM qd_auto_bots WHERE user_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3"#,
    )
    .bind(user_id)
    .bind(page_size)
    .bind(offset)
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_default();

    Json(ApiResponse::ok(serde_json::json!({
        "total": count.0,
        "page": page,
        "page_size": page_size,
        "bots": bots,
    })))
}

pub async fn get_bot(
    State(state): State<std::sync::Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Json<ApiResponse<serde_json::Value>> {
    let user_id = get_user_id(&auth);

    let bot: Option<crate::bot::auto_trade::types::AutoBot> = sqlx::query_as(
        "SELECT * FROM qd_auto_bots WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await
    .unwrap_or(None);

    match bot {
        Some(b) => {
            let trades: Vec<crate::bot::auto_trade::types::AutoTrade> = sqlx::query_as(
                "SELECT * FROM qd_auto_trades WHERE bot_id = $1 ORDER BY created_at DESC LIMIT 50",
            )
            .bind(id)
            .fetch_all(&state.db_pool)
            .await
            .unwrap_or_default();

            Json(ApiResponse::ok(serde_json::json!({
                "bot": b,
                "trades": trades,
            })))
        }
        None => Json(ApiResponse::err("Auto bot not found")),
    }
}

pub async fn start_bot(
    State(state): State<std::sync::Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Json<ApiResponse<serde_json::Value>> {
    let user_id = get_user_id(&auth);

    let bot: Option<crate::bot::auto_trade::types::AutoBot> = sqlx::query_as(
        "SELECT * FROM qd_auto_bots WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await
    .unwrap_or(None);

    if bot.is_none() {
        return Json(ApiResponse::err("Auto bot not found"));
    }

    let bot = bot.unwrap();
    let mt = match bot.market_type.as_str() {
        "spot" => MarketType::Spot,
        _ => MarketType::Perpetual,
    };
    let _ = super::market::ensure_exchange(&state, &bot.exchange, mt).await;

    if let Some(ref auto_cmd_tx) = state.auto_cmd_tx {
        if let Err(e) = auto_cmd_tx.send(crate::bot::auto_trade::types::AutoCommand::StartBot { bot_id: id }).await {
            return Json(ApiResponse::err(format!("Failed to start auto engine: {}", e)));
        }
    }

    Json(ApiResponse::ok(serde_json::json!({ "started": true })))
}

pub async fn stop_bot(
    State(state): State<std::sync::Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Json<ApiResponse<serde_json::Value>> {
    let user_id = get_user_id(&auth);

    let bot: Option<crate::bot::auto_trade::types::AutoBot> = sqlx::query_as(
        "SELECT * FROM qd_auto_bots WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await
    .unwrap_or(None);

    if bot.is_none() {
        return Json(ApiResponse::err("Auto bot not found"));
    }

    if let Some(ref auto_cmd_tx) = state.auto_cmd_tx {
        if let Err(e) = auto_cmd_tx.send(crate::bot::auto_trade::types::AutoCommand::StopBot { bot_id: id }).await {
            return Json(ApiResponse::err(format!("Failed to stop auto engine: {}", e)));
        }
    }

    Json(ApiResponse::ok(serde_json::json!({ "stopped": true })))
}

pub async fn delete_bot(
    State(state): State<std::sync::Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Json<ApiResponse<serde_json::Value>> {
    let user_id = get_user_id(&auth);

    let bot: Option<crate::bot::auto_trade::types::AutoBot> = sqlx::query_as(
        "SELECT * FROM qd_auto_bots WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await
    .unwrap_or(None);

    if bot.is_none() {
        return Json(ApiResponse::err("Auto bot not found"));
    }

    if let Some(ref auto_cmd_tx) = state.auto_cmd_tx {
        let _ = auto_cmd_tx.send(crate::bot::auto_trade::types::AutoCommand::DeleteBot {
            bot_id: id,
            close_position: true,
        }).await;
    } else {
        let _ = sqlx::query("DELETE FROM qd_auto_bots WHERE id = $1")
            .bind(id)
            .execute(&state.db_pool)
            .await;
    }

    Json(ApiResponse::ok(serde_json::json!({ "deleted": true })))
}

#[derive(Debug, Deserialize)]
pub struct GetTradesQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

pub async fn get_trades(
    State(state): State<std::sync::Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Query(query): Query<GetTradesQuery>,
) -> Json<ApiResponse<serde_json::Value>> {
    let user_id = get_user_id(&auth);

    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).min(100);
    let offset = (page - 1) * page_size;

    let bot_check: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM qd_auto_bots WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await
    .unwrap_or(None);

    if bot_check.is_none() {
        return Json(ApiResponse::err("Auto bot not found"));
    }

    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM qd_auto_trades WHERE bot_id = $1",
    )
    .bind(id)
    .fetch_one(&state.db_pool)
    .await
    .unwrap_or((0,));

    let trades: Vec<crate::bot::auto_trade::types::AutoTrade> = sqlx::query_as(
        r#"SELECT * FROM qd_auto_trades WHERE bot_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3"#,
    )
    .bind(id)
    .bind(page_size)
    .bind(offset)
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_default();

    Json(ApiResponse::ok(serde_json::json!({
        "total": count.0,
        "page": page,
        "page_size": page_size,
        "trades": trades,
    })))
}

#[derive(Debug, Deserialize)]
pub struct AnalysisLogsQuery {
    pub bot_id: Uuid,
}

pub async fn get_analysis_logs(
    State(state): State<std::sync::Arc<AppState>>,
    auth: AuthUser,
    Query(query): Query<AnalysisLogsQuery>,
) -> Json<ApiResponse<serde_json::Value>> {
    let user_id = get_user_id(&auth);

    let bot_check: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM qd_auto_bots WHERE id = $1 AND user_id = $2",
    )
    .bind(query.bot_id)
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await
    .unwrap_or(None);

    if bot_check.is_none() {
        return Json(ApiResponse::err("Auto bot not found"));
    }

    #[derive(Debug, sqlx::FromRow, serde::Serialize)]
    struct LogRow {
        id: Uuid,
        bot_id: Uuid,
        analysis_type: String,
        status: String,
        system_prompt: String,
        user_prompt: String,
        result: serde_json::Value,
        error: Option<String>,
        created_at: chrono::DateTime<chrono::Utc>,
    }

    let logs: Vec<LogRow> = sqlx::query_as(
        r#"SELECT id, bot_id, analysis_type, status, system_prompt, user_prompt, result, error, created_at
           FROM qd_auto_analysis_logs WHERE bot_id = $1 ORDER BY created_at DESC LIMIT 50"#,
    )
    .bind(query.bot_id)
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_default();

    Json(ApiResponse::ok(serde_json::json!({
        "logs": logs,
    })))
}
