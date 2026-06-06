//! Dashboard handlers.

use axum::{
    extract::State,
    Json,
};

use crate::handlers::auth::ApiResponse;
use crate::state::AppState;

pub async fn dashboard_summary(
    State(state): State<AppState>,
) -> Json<ApiResponse> {
    let grid_total: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM qd_grid_bots"#)
        .fetch_one(&state.db_pool)
        .await
        .unwrap_or(0);

    let grid_running: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM qd_grid_bots WHERE status = 'running'"#)
        .fetch_one(&state.db_pool)
        .await
        .unwrap_or(0);

    let auto_total: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM qd_auto_bots"#)
        .fetch_one(&state.db_pool)
        .await
        .unwrap_or(0);

    let auto_running: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM qd_auto_bots WHERE status = 'running'"#)
        .fetch_one(&state.db_pool)
        .await
        .unwrap_or(0);

    let trade_total: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM qd_grid_trades"#)
        .fetch_one(&state.db_pool)
        .await
        .unwrap_or(0)
        + sqlx::query_scalar(r#"SELECT COUNT(*) FROM qd_auto_trades"#)
            .fetch_one(&state.db_pool)
            .await
            .unwrap_or(0);

    let grid_pnl: f64 = sqlx::query_scalar(r#"SELECT COALESCE(SUM(pnl), 0) FROM qd_grid_trades WHERE status = 'closed'"#)
        .fetch_one(&state.db_pool)
        .await
        .unwrap_or(0.0);

    let auto_pnl: f64 = sqlx::query_scalar(r#"SELECT COALESCE(SUM(pnl), 0) FROM qd_auto_trades"#)
        .fetch_one(&state.db_pool)
        .await
        .unwrap_or(0.0);

    let bot_total = grid_total + auto_total;
    let bot_running = grid_running + auto_running;

    Json(ApiResponse::ok(serde_json::json!({
        "bots": {
            "total": bot_total,
            "running": bot_running,
            "stopped": bot_total - bot_running,
        },
        "trades": {
            "total": trade_total,
            "total_pnl": grid_pnl + auto_pnl,
        },
        "exchanges": ["binance", "bybit", "okx"],
        "paper_mode": state.paper_mode,
    })))
}

pub async fn list_positions(
    State(state): State<AppState>,
) -> Json<ApiResponse> {
    // Grid bots with open trades
    let grid_rows = sqlx::query_as::<_, (String, String, String, f64, f64, chrono::DateTime<chrono::Utc>)>(
        r#"SELECT symbol, exchange, open_side, open_price, open_quantity, opened_at
           FROM qd_grid_trades WHERE status = 'open' ORDER BY opened_at DESC"#,
    )
    .fetch_all(&state.db_pool)
    .await;

    // Auto bots with active positions
    let auto_rows = sqlx::query_as::<_, (String, String, String, f64, f64, chrono::DateTime<chrono::Utc>)>(
        r#"SELECT symbol, exchange, current_side, entry_price, position_size, COALESCE(started_at, created_at)
           FROM qd_auto_bots WHERE status = 'running' AND current_side IS NOT NULL AND current_side != 'none' AND position_size > 0
           ORDER BY updated_at DESC"#,
    )
    .fetch_all(&state.db_pool)
    .await;

    let mut positions = Vec::new();

    if let Ok(rows) = grid_rows {
        for (symbol, exchange, side, entry_price, size, opened_at) in rows {
            positions.push(serde_json::json!({
                "source": "grid",
                "symbol": symbol,
                "exchange": exchange,
                "side": side,
                "entry_price": entry_price,
                "size": size,
                "opened_at": opened_at.to_rfc3339(),
            }));
        }
    }

    if let Ok(rows) = auto_rows {
        for (symbol, exchange, side, entry_price, size, opened_at) in rows {
            positions.push(serde_json::json!({
                "source": "auto",
                "symbol": symbol,
                "exchange": exchange,
                "side": side,
                "entry_price": entry_price,
                "size": size,
                "opened_at": opened_at.to_rfc3339(),
            }));
        }
    }

    Json(ApiResponse::ok(serde_json::json!({ "positions": positions })))
}

pub async fn list_trades(
    State(state): State<AppState>,
) -> Json<ApiResponse> {
    // Grid trades - split into two queries to avoid FromRow tuple limit
    let grid_open = sqlx::query_as::<_, (String, String, i32, String, f64, f64, chrono::DateTime<chrono::Utc>, f64, f64, String)>(
        r#"SELECT symbol, exchange, grid_level, open_side, open_price, open_quantity, opened_at, pnl, pnl_pct, status
           FROM qd_grid_trades WHERE status = 'open' ORDER BY opened_at DESC LIMIT 50"#,
    )
    .fetch_all(&state.db_pool)
    .await;

    let grid_closed = sqlx::query_as::<_, (String, String, i32, String, f64, f64, chrono::DateTime<chrono::Utc>, Option<String>, Option<f64>, Option<f64>, Option<chrono::DateTime<chrono::Utc>>, f64, f64)>(
        r#"SELECT symbol, exchange, grid_level, open_side, open_price, open_quantity, opened_at, close_side, close_price, close_quantity, closed_at, pnl, pnl_pct
           FROM qd_grid_trades WHERE status = 'closed' ORDER BY closed_at DESC LIMIT 50"#,
    )
    .fetch_all(&state.db_pool)
    .await;

    // Auto trades
    let auto_rows = sqlx::query_as::<_, (String, String, String, String, f64, f64, f64, f64, chrono::DateTime<chrono::Utc>)>(
        r#"SELECT symbol, exchange, side, trade_type, price, quantity, pnl, pnl_pct, created_at
           FROM qd_auto_trades ORDER BY created_at DESC LIMIT 50"#,
    )
    .fetch_all(&state.db_pool)
    .await;

    let mut trades = Vec::new();

    if let Ok(rows) = grid_open {
        for (symbol, exchange, grid_level, open_side, open_price, open_quantity, opened_at, pnl, pnl_pct, status) in rows {
            trades.push(serde_json::json!({
                "symbol": symbol,
                "exchange": exchange,
                "grid_level": grid_level,
                "open_side": open_side,
                "open_price": open_price,
                "open_quantity": open_quantity,
                "opened_at": opened_at.to_rfc3339(),
                "close_side": null,
                "close_price": null,
                "close_quantity": null,
                "closed_at": null,
                "pnl": pnl,
                "pnl_pct": pnl_pct,
                "status": status,
                "source": "grid",
            }));
        }
    }

    if let Ok(rows) = grid_closed {
        for (symbol, exchange, grid_level, open_side, open_price, open_quantity, opened_at, close_side, close_price, close_quantity, closed_at, pnl, pnl_pct) in rows {
            trades.push(serde_json::json!({
                "symbol": symbol,
                "exchange": exchange,
                "grid_level": grid_level,
                "open_side": open_side,
                "open_price": open_price,
                "open_quantity": open_quantity,
                "opened_at": opened_at.to_rfc3339(),
                "close_side": close_side,
                "close_price": close_price,
                "close_quantity": close_quantity,
                "closed_at": closed_at.map(|t| t.to_rfc3339()),
                "pnl": pnl,
                "pnl_pct": pnl_pct,
                "status": "closed",
                "source": "grid",
            }));
        }
    }

    if let Ok(rows) = auto_rows {
        for (symbol, exchange, side, trade_type, price, quantity, pnl, pnl_pct, created_at) in rows {
            trades.push(serde_json::json!({
                "symbol": symbol,
                "exchange": exchange,
                "side": side,
                "type": trade_type,
                "price": price,
                "quantity": quantity,
                "pnl": pnl,
                "pnl_pct": pnl_pct,
                "created_at": created_at.to_rfc3339(),
                "source": "auto",
            }));
        }
    }

    // Sort by time descending
    trades.sort_by(|a, b| {
        let a_time = a.get("opened_at")
            .or_else(|| a.get("created_at"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let b_time = b.get("opened_at")
            .or_else(|| b.get("created_at"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        b_time.cmp(a_time)
    });

    Json(ApiResponse::ok(serde_json::json!({ "items": trades })))
}

pub async fn list_pending_orders(
    State(state): State<AppState>,
) -> Json<ApiResponse> {
    let rows = sqlx::query_as::<_, (String, String, String, Option<f64>, f64, chrono::DateTime<chrono::Utc>)>(
        r#"SELECT symbol, side, order_type, price, amount, created_at
           FROM pending_orders WHERE status = 'pending' ORDER BY created_at DESC LIMIT 100"#,
    )
    .fetch_all(&state.db_pool)
    .await;

    match rows {
        Ok(orders) => Json(ApiResponse::ok(serde_json::json!({
            "orders": orders.iter().map(|(symbol, side, order_type, price, amount, created_at)| {
                serde_json::json!({
                    "symbol": symbol,
                    "side": side,
                    "type": order_type,
                    "price": price,
                    "amount": amount,
                    "created_at": created_at.to_rfc3339(),
                })
            }).collect::<Vec<_>>()
        }))),
        Err(e) => Json(ApiResponse::err(format!("Database error: {}", e))),
    }
}
