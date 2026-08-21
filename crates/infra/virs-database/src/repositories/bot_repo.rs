use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tracing::warn;
use uuid::Uuid;
use virs_error::VirsResult;
use virs_type::{Bot, BotConfig, BotStore};

use crate::models::BotTradeRow;

pub struct PgBotStore {
    db: PgPool,
}

impl PgBotStore {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    /* BotStore trait方法用到的内部连接池引用 */
    pub fn pool(&self) -> &PgPool {
        &self.db
    }
}

/* Bot到BotConfig的转换：将数据库模型转换为引擎使用的配置对象 */
pub fn bot_to_config(bot: &Bot) -> BotConfig {
    BotConfig {
        id: bot.id,
        user_id: bot.user_id,
        name: bot.name.clone(),
        symbol: bot.symbol.clone(),
        exchange: bot.exchange.clone(),
        paper_mode: bot.paper_mode,
        leverage: bot.leverage,
        max_position_pct: bot.max_position_pct,
        decide_interval_secs: bot.decide_interval_secs,
        position_id_long: bot.position_id_long,
        position_id_short: bot.position_id_short,
        market_regime: bot.market_regime.clone(),
        ai_analysis: bot.ai_analysis.clone(),
        system_prompt: bot.system_prompt.clone(),
        user_prompt: bot.user_prompt.clone(),
        total_pnl: bot.total_pnl,
        total_trades: bot.total_trades,
        win_trades: bot.win_trades,
        loss_trades: bot.loss_trades,
        last_decided_at: bot.last_decided_at,
        strategy_file: bot.strategy_file.clone(),
        auto_optimize_enabled: bot.auto_optimize_enabled,
    }
}

#[async_trait]
impl BotStore for PgBotStore {
    async fn load_running_bots(&self) -> VirsResult<Vec<BotConfig>> {
        let bots: Vec<Bot> =
            sqlx::query_as("SELECT * FROM qd_bots WHERE status = 'running'")
                .fetch_all(&self.db)
                .await?;
        Ok(bots.iter().map(bot_to_config).collect())
    }

    async fn load_bot(&self, bot_id: Uuid) -> VirsResult<Option<BotConfig>> {
        let bot: Option<Bot> = sqlx::query_as("SELECT * FROM qd_bots WHERE id = $1")
            .bind(bot_id)
            .fetch_optional(&self.db)
            .await?;
        Ok(bot.as_ref().map(bot_to_config))
    }

    async fn update_bot_status(&self, bot_id: Uuid, status: &str) -> VirsResult<()> {
        let sql = match status {
            "running" => "UPDATE qd_bots SET status = 'running', started_at = NOW(), updated_at = NOW() WHERE id = $1",
            "stopped" => "UPDATE qd_bots SET status = 'stopped', stopped_at = NOW(), updated_at = NOW() WHERE id = $1",
            "paused" => "UPDATE qd_bots SET status = 'paused', updated_at = NOW() WHERE id = $1",
            _ => "UPDATE qd_bots SET status = $2, updated_at = NOW() WHERE id = $1",
        };
        if status == "running" || status == "stopped" || status == "paused" {
            sqlx::query(sql).bind(bot_id).execute(&self.db).await?;
        } else {
            sqlx::query(sql)
                .bind(bot_id)
                .bind(status)
                .execute(&self.db)
                .await?;
        }
        Ok(())
    }

    async fn update_last_decided(&self, bot_id: Uuid) -> VirsResult<()> {
        sqlx::query("UPDATE qd_bots SET last_decided_at = NOW() WHERE id = $1")
            .bind(bot_id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    async fn update_position(
        &self,
        bot_id: Uuid,
        position_id_long: Option<Uuid>,
        position_id_short: Option<Uuid>,
    ) -> VirsResult<()> {
        sqlx::query(
            r#"UPDATE qd_bots SET
                position_id_long = $2, position_id_short = $3, updated_at = NOW()
               WHERE id = $1"#,
        )
        .bind(bot_id)
        .bind(position_id_long)
        .bind(position_id_short)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn update_ai_analysis(
        &self,
        bot_id: Uuid,
        market_regime: &str,
        leverage: i32,
        ai_analysis: &str,
    ) -> VirsResult<()> {
        sqlx::query(
            r#"UPDATE qd_bots SET
                market_regime = $2, leverage = $3, ai_analysis = $4, updated_at = NOW()
               WHERE id = $1"#,
        )
        .bind(bot_id)
        .bind(market_regime)
        .bind(leverage)
        .bind(ai_analysis)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn update_stats(
        &self,
        bot_id: Uuid,
        total_pnl: f64,
        total_trades: i32,
        win_trades: i32,
        loss_trades: i32,
    ) -> VirsResult<()> {
        sqlx::query(
            r#"UPDATE qd_bots SET
                total_pnl = $2, total_trades = $3, win_trades = $4, loss_trades = $5, updated_at = NOW()
               WHERE id = $1"#,
        )
        .bind(bot_id).bind(total_pnl).bind(total_trades).bind(win_trades).bind(loss_trades)
        .execute(&self.db).await?;
        Ok(())
    }

    async fn record_open_trade(
        &self,
        bot_id: Uuid,
        user_id: Uuid,
        symbol: &str,
        exchange: &str,
        client_order_id: &str,
        stop_loss: f64,
        take_profit: f64,
        strategy_file: &Option<String>,
    ) -> VirsResult<()> {
        sqlx::query(
            r#"INSERT INTO pe_bot_order_context
               (client_order_id, bot_id, user_id, symbol, exchange, order_role, status, stop_loss, take_profit, strategy_file)
               VALUES ($1, $2, $3, $4, $5, 'open', 'open', $6, $7, $8)"#,
        )
        .bind(client_order_id)
        .bind(bot_id)
        .bind(user_id)
        .bind(symbol)
        .bind(exchange)
        .bind(stop_loss)
        .bind(take_profit)
        .bind(strategy_file)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn close_trade(
        &self,
        open_client_order_id: &str,
        close_client_order_id: &str,
        close_reason: &str,
    ) -> VirsResult<()> {
        let result = sqlx::query(
            r#"UPDATE pe_bot_order_context SET status = 'closed'
               WHERE client_order_id = $1 AND order_role = 'open' AND status = 'open'"#,
        )
        .bind(open_client_order_id)
        .execute(&self.db)
        .await?;

        if result.rows_affected() == 0 {
            warn!(open_client_order_id = %open_client_order_id, "No open trade found for close");
        }

        sqlx::query(
            r#"INSERT INTO pe_bot_order_context
               (client_order_id, bot_id, user_id, symbol, exchange, order_role, status, paired_client_order_id, close_reason, strategy_file)
               SELECT $1, bot_id, user_id, symbol, exchange, 'close', 'closed', client_order_id, $2, strategy_file
               FROM pe_bot_order_context
               WHERE client_order_id = $3 AND order_role = 'open'"#,
        )
        .bind(close_client_order_id)
        .bind(close_reason)
        .bind(open_client_order_id)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    async fn find_open_trade(
        &self,
        bot_id: Uuid,
    ) -> VirsResult<Option<(String, f64, f64, DateTime<Utc>)>> {
        let row: Option<(String, f64, f64, DateTime<Utc>)> = sqlx::query_as(
            r#"SELECT client_order_id, stop_loss, take_profit, created_at
               FROM pe_bot_order_context
               WHERE bot_id = $1 AND order_role = 'open' AND status = 'open'
               ORDER BY created_at DESC LIMIT 1"#,
        )
        .bind(bot_id)
        .fetch_optional(&self.db)
        .await?;
        Ok(row)
    }

    async fn mark_trade_orphaned(&self, client_order_id: &str) -> VirsResult<()> {
        sqlx::query(
            r#"UPDATE pe_bot_order_context SET status = 'orphaned'
               WHERE client_order_id = $1 AND status = 'open'"#,
        )
        .bind(client_order_id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn find_last_closed_trade(
        &self,
        bot_id: Uuid,
    ) -> VirsResult<Option<(String, String, DateTime<Utc>)>> {
        let row: Option<(String, String, DateTime<Utc>)> = sqlx::query_as(
            r#"SELECT LOWER(open_ord.position_side) AS open_side,
                      COALESCE(close_ctx.close_reason, '') AS close_reason,
                      close_ctx.created_at
               FROM pe_bot_order_context close_ctx
               JOIN pe_bot_order_context open_ctx ON open_ctx.client_order_id = close_ctx.paired_client_order_id
               JOIN pe_order_latest open_ord ON open_ord.client_order_id = open_ctx.client_order_id
               WHERE close_ctx.bot_id = $1 AND close_ctx.order_role = 'close' AND close_ctx.status = 'closed'
               ORDER BY close_ctx.created_at DESC LIMIT 1"#,
        )
        .bind(bot_id)
        .fetch_optional(&self.db)
        .await?;
        Ok(row)
    }

    async fn record_orphaned_close_trade(
        &self,
        bot_id: Uuid,
        user_id: Uuid,
        symbol: &str,
        exchange: &str,
        close_client_order_id: &str,
        close_reason: &str,
        strategy_file: &Option<String>,
    ) -> VirsResult<()> {
        sqlx::query(
            r#"INSERT INTO pe_bot_order_context
               (client_order_id, bot_id, user_id, symbol, exchange, order_role, status, close_reason, strategy_file)
               VALUES ($1, $2, $3, $4, $5, 'close', 'orphaned', $6, $7)"#,
        )
        .bind(close_client_order_id)
        .bind(bot_id)
        .bind(user_id)
        .bind(symbol)
        .bind(exchange)
        .bind(close_reason)
        .bind(strategy_file)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn save_analysis_log(
        &self,
        bot_id: Uuid,
        analysis_type: &str,
        system_prompt: &str,
        user_prompt: &str,
        result: &serde_json::Value,
        error: Option<&str>,
        llm_model: &str,
        strategy_file: &Option<String>,
    ) -> VirsResult<Uuid> {
        let status = if error.is_some() {
            "failed"
        } else {
            "completed"
        };
        let row: (Uuid,) = sqlx::query_as(
            r#"INSERT INTO qd_bot_analysis_logs (bot_id, analysis_type, system_prompt, user_prompt, status, result, error, llm_model, completed_at, strategy_file)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW(), $9)
               RETURNING id"#,
        )
        .bind(bot_id).bind(analysis_type).bind(system_prompt).bind(user_prompt)
        .bind(status).bind(result).bind(error).bind(llm_model)
        .bind(strategy_file)
        .fetch_one(&self.db).await?;
        Ok(row.0)
    }

    async fn update_analysis_log_execution(
        &self,
        log_id: Uuid,
        execution_status: &str,
        intercept_reason: Option<&str>,
    ) -> VirsResult<()> {
        let status = if intercept_reason.is_some() {
            "intercepted"
        } else {
            "completed"
        };
        sqlx::query(
            r#"UPDATE qd_bot_analysis_logs SET
               execution_status = $2,
               intercept_reason = $3,
               status = $4
               WHERE id = $1"#,
        )
        .bind(log_id)
        .bind(execution_status)
        .bind(intercept_reason)
        .bind(status)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn load_consecutive_losses(&self, bot_id: Uuid) -> VirsResult<i32> {
        let pnl_rows: Vec<(f64,)> = sqlx::query_as(
            r#"SELECT close_ord.realized_pnl::float AS pnl
               FROM pe_bot_order_context close_ctx
               JOIN pe_order_latest close_ord ON close_ord.client_order_id = close_ctx.client_order_id
               WHERE close_ctx.bot_id = $1 AND close_ctx.order_role = 'close' AND close_ctx.status = 'closed'
               ORDER BY close_ctx.created_at DESC LIMIT 20"#,
        )
        .bind(bot_id)
        .fetch_all(&self.db)
        .await?;

        let mut count = 0i32;
        for (pnl,) in &pnl_rows {
            if *pnl < 0.0 {
                count += 1;
            } else {
                break;
            }
        }
        Ok(count)
    }

    async fn delete_bot(&self, bot_id: Uuid) -> VirsResult<()> {
        sqlx::query("DELETE FROM qd_bots WHERE id = $1")
            .bind(bot_id)
            .execute(&self.db)
            .await?;
        Ok(())
    }
}

/* ========== engine_manager.rs 使用的查询 ========== */

/* 查询bots总数：用于判断是否需要执行重启恢复 */
pub async fn count_all_bots(pool: &PgPool) -> VirsResult<i64> {
    let count: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM qd_bots"#)
        .fetch_one(pool)
        .await?;
    Ok(count)
}

/* 查询所有running bot的exchange和symbol：用于恢复K线/订单簿订阅 */
pub async fn get_running_bot_symbols(pool: &PgPool) -> VirsResult<Vec<(String, String)>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        r#"SELECT exchange, symbol FROM qd_bots WHERE status = 'running'"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/* 查询running bot的paper_mode去重列表：用于确定引擎模式 */
pub async fn get_running_paper_modes(pool: &PgPool) -> VirsResult<Vec<bool>> {
    let modes: Vec<bool> = sqlx::query_scalar(
        r#"SELECT DISTINCT paper_mode FROM qd_bots WHERE status = 'running'"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(modes)
}

/* 恢复失败时将所有running bot标记为error状态 */
pub async fn mark_running_bots_as_error(pool: &PgPool) -> VirsResult<()> {
    sqlx::query(r#"UPDATE qd_bots SET status = 'error', stopped_at = NOW() WHERE status = 'running'"#)
        .execute(pool)
        .await?;
    Ok(())
}

/* ========== bot_trade.rs handler 使用的查询 ========== */

/* 查询指定用户的bot数量：用于限制每用户只能创建一个bot */
pub async fn count_bots_by_user(pool: &PgPool, user_id: Uuid) -> VirsResult<i64> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM qd_bots WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    Ok(count)
}

/* 插入新bot：状态默认为stopped，创建时间和更新时间由数据库填充 */
pub async fn insert_bot(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
    name: &str,
    symbol: &str,
    exchange: &str,
    leverage: i32,
    max_position_pct: f64,
    decide_interval_secs: i32,
    paper_mode: bool,
    initial_capital: f64,
    bot_type: &str,
    strategy_file: &str,
    auto_optimize_enabled: bool,
) -> VirsResult<()> {
    sqlx::query(
        r#"INSERT INTO qd_bots (id, user_id, name, symbol, exchange, leverage, max_position_pct, decide_interval_secs, paper_mode, initial_capital, status, bot_type, strategy_file, auto_optimize_enabled, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'stopped', $11, $12, $13, NOW(), NOW())"#,
    )
    .bind(id)
    .bind(user_id)
    .bind(name)
    .bind(symbol)
    .bind(exchange)
    .bind(leverage)
    .bind(max_position_pct)
    .bind(decide_interval_secs)
    .bind(paper_mode)
    .bind(initial_capital)
    .bind(bot_type)
    .bind(strategy_file)
    .bind(auto_optimize_enabled)
    .execute(pool)
    .await?;
    Ok(())
}

/* 插入策略选择日志：记录LLM自动选择策略的结果 */
pub async fn insert_strategy_selection_log(
    pool: &PgPool,
    bot_id: Uuid,
    system_prompt: &str,
    user_prompt: &str,
    result: &serde_json::Value,
    strategy_file: &str,
) -> VirsResult<()> {
    sqlx::query(
        r#"INSERT INTO qd_bot_analysis_logs (bot_id, analysis_type, system_prompt, user_prompt, status, result, strategy_file, completed_at)
           VALUES ($1, 'strategy_selection', $2, $3, 'completed', $4, $5, NOW())"#,
    )
    .bind(bot_id)
    .bind(system_prompt)
    .bind(user_prompt)
    .bind(result)
    .bind(strategy_file)
    .execute(pool)
    .await?;
    Ok(())
}

/* 查询指定用户的所有bot：按创建时间倒序排列 */
pub async fn list_bots_by_user(pool: &PgPool, user_id: Uuid) -> VirsResult<Vec<Bot>> {
    let bots: Vec<Bot> =
        sqlx::query_as("SELECT * FROM qd_bots WHERE user_id = $1 ORDER BY created_at DESC")
            .bind(user_id)
            .fetch_all(pool)
            .await?;
    Ok(bots)
}

/* 按ID和user_id查询单个bot：用于get_bot和update_bot等需要所有权验证的场景 */
pub async fn get_bot_by_id(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
) -> VirsResult<Option<Bot>> {
    let bot: Option<Bot> =
        sqlx::query_as("SELECT * FROM qd_bots WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
    Ok(bot)
}

/* 更新bot的strategy_file：仅停止状态下允许更新 */
pub async fn update_bot_strategy(
    pool: &PgPool,
    id: Uuid,
    new_strategy_file: &str,
    user_id: Uuid,
) -> VirsResult<()> {
    sqlx::query("UPDATE qd_bots SET strategy_file = $2, updated_at = NOW() WHERE id = $1 AND user_id = $3")
        .bind(id)
        .bind(new_strategy_file)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

/* 验证bot所有权：检查指定bot是否属于指定用户 */
pub async fn verify_bot_ownership(
    pool: &PgPool,
    bot_id: Uuid,
    user_id: Uuid,
) -> VirsResult<bool> {
    let exists: Option<bool> =
        sqlx::query_scalar("SELECT true FROM qd_bots WHERE id = $1 AND user_id = $2")
            .bind(bot_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
    Ok(exists.is_some())
}

/* 统计bot的开仓交易数量：用于交易列表分页 */
pub async fn count_bot_trades(
    pool: &PgPool,
    bot_id: Uuid,
    user_id: Uuid,
) -> VirsResult<i64> {
    let total: i64 = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*) FROM pe_bot_order_context WHERE bot_id = $1 AND user_id = $2 AND order_role = 'open'"#,
    )
    .bind(bot_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(total)
}

/* 查询bot交易详情列表：关联开仓和平仓订单上下文，支持分页 */
pub async fn query_bot_trades(
    pool: &PgPool,
    bot_id: Uuid,
    user_id: Uuid,
    limit: i64,
    offset: i64,
) -> VirsResult<Vec<BotTradeRow>> {
    let trades: Vec<BotTradeRow> = sqlx::query_as(
        r#"SELECT
             open_ctx.client_order_id AS open_client_order_id,
             close_ctx.client_order_id AS close_client_order_id,
             open_ctx.bot_id,
             open_ctx.symbol,
             open_ctx.exchange,
             LOWER(open_ord.side) AS open_side,
             open_ord.avg_fill_price::float AS open_price,
             open_ord.filled_qty::float AS open_quantity,
             open_ord.commission::float AS open_fee,
             open_ctx.created_at AS opened_at,
             CASE WHEN close_ord.side IS NOT NULL THEN LOWER(close_ord.side) END AS close_side,
             close_ord.avg_fill_price::float AS close_price,
             close_ord.filled_qty::float AS close_quantity,
             COALESCE(close_ord.commission::float, 0) AS close_fee,
             close_ctx.created_at AS closed_at,
             COALESCE(close_ord.realized_pnl::float, 0) AS pnl,
             open_ctx.stop_loss,
             open_ctx.take_profit,
             close_ctx.close_reason,
             open_ctx.status
           FROM pe_bot_order_context open_ctx
           JOIN pe_order_latest open_ord ON open_ord.client_order_id = open_ctx.client_order_id
           LEFT JOIN pe_bot_order_context close_ctx ON close_ctx.paired_client_order_id = open_ctx.client_order_id
           LEFT JOIN pe_order_latest close_ord ON close_ord.client_order_id = close_ctx.client_order_id
           WHERE open_ctx.bot_id = $1 AND open_ctx.user_id = $2 AND open_ctx.order_role = 'open'
           ORDER BY open_ctx.created_at DESC LIMIT $3 OFFSET $4"#,
    )
    .bind(bot_id)
    .bind(user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(trades)
}

/* 查询bot已平仓交易的统计数据：用于计算胜率、盈亏比、最大回撤等指标 */
pub async fn get_bot_trade_stats(
    pool: &PgPool,
    bot_id: Uuid,
    user_id: Uuid,
) -> VirsResult<Vec<(f64, f64, f64, f64, f64, DateTime<Utc>, DateTime<Utc>)>> {
    let trades: Vec<(f64, f64, f64, f64, f64, DateTime<Utc>, DateTime<Utc>)> = sqlx::query_as(
        r#"SELECT
             open_ord.avg_fill_price::float AS open_price,
             open_ord.filled_qty::float AS open_quantity,
             open_ord.commission::float AS open_fee,
             COALESCE(close_ord.commission::float, 0) AS close_fee,
             COALESCE(close_ord.realized_pnl::float, 0) AS pnl,
             open_ctx.created_at AS opened_at,
             close_ctx.created_at AS closed_at
           FROM pe_bot_order_context open_ctx
           JOIN pe_order_latest open_ord ON open_ord.client_order_id = open_ctx.client_order_id
           JOIN pe_bot_order_context close_ctx ON close_ctx.paired_client_order_id = open_ctx.client_order_id AND close_ctx.order_role = 'close'
           JOIN pe_order_latest close_ord ON close_ord.client_order_id = close_ctx.client_order_id
           WHERE open_ctx.bot_id = $1 AND open_ctx.user_id = $2 AND open_ctx.status = 'closed'
           ORDER BY close_ctx.created_at ASC"#,
    )
    .bind(bot_id)
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(trades)
}

/* 统计bot的分析日志数量：用于日志列表分页 */
pub async fn count_analysis_logs(
    pool: &PgPool,
    bot_id: Uuid,
    user_id: Uuid,
) -> VirsResult<i64> {
    let total: i64 = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*) FROM qd_bot_analysis_logs l
           JOIN qd_bots b ON l.bot_id = b.id
           WHERE l.bot_id = $1 AND b.user_id = $2"#,
    )
    .bind(bot_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(total)
}

/* 查询bot的分析日志列表：关联qd_bots验证用户所有权，支持分页 */
pub async fn query_analysis_logs(
    pool: &PgPool,
    bot_id: Uuid,
    user_id: Uuid,
    limit: i64,
    offset: i64,
) -> VirsResult<Vec<(Uuid, Uuid, String, String, String, serde_json::Value, Option<String>, String, String, DateTime<Utc>, Option<String>, Option<String>, Option<String>, Option<DateTime<Utc>>)>> {
    let logs: Vec<(Uuid, Uuid, String, String, String, serde_json::Value, Option<String>, String, String, DateTime<Utc>, Option<String>, Option<String>, Option<String>, Option<DateTime<Utc>>)> = sqlx::query_as(
        r#"SELECT l.id, l.bot_id, l.analysis_type, l.status, l.system_prompt, l.result, l.error, l.user_prompt, l.llm_model, l.created_at, l.strategy_file, l.execution_status, l.intercept_reason, l.completed_at
           FROM qd_bot_analysis_logs l
           JOIN qd_bots b ON l.bot_id = b.id
           WHERE l.bot_id = $1 AND b.user_id = $2
           ORDER BY l.created_at DESC LIMIT $3 OFFSET $4"#,
    )
    .bind(bot_id)
    .bind(user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(logs)
}
