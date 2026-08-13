use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tracing::warn;
use uuid::Uuid;
use virs_error::VirsResult;

use virs_type::ChatBot;
use virs_type::ChatBotConfig;
use virs_type::*;

pub struct PgChatStore {
    db: PgPool,
}

impl PgChatStore {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }
}

/* ChatBot到ChatBotConfig的转换：将数据库模型转换为引擎使用的配置对象 */
pub fn bot_to_config(bot: &ChatBot) -> ChatBotConfig {
    ChatBotConfig {
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
impl ChatStore for PgChatStore {
    async fn load_running_bots(&self) -> VirsResult<Vec<ChatBotConfig>> {
        let bots: Vec<ChatBot> =
            sqlx::query_as("SELECT * FROM qd_chat_bots WHERE status = 'running'")
                .fetch_all(&self.db)
                .await?;
        Ok(bots.iter().map(bot_to_config).collect())
    }

    async fn load_bot(&self, bot_id: Uuid) -> VirsResult<Option<ChatBotConfig>> {
        let bot: Option<ChatBot> = sqlx::query_as("SELECT * FROM qd_chat_bots WHERE id = $1")
            .bind(bot_id)
            .fetch_optional(&self.db)
            .await?;
        Ok(bot.as_ref().map(bot_to_config))
    }

    async fn update_bot_status(&self, bot_id: Uuid, status: &str) -> VirsResult<()> {
        let sql = match status {
            "running" => "UPDATE qd_chat_bots SET status = 'running', started_at = NOW(), updated_at = NOW() WHERE id = $1",
            "stopped" => "UPDATE qd_chat_bots SET status = 'stopped', stopped_at = NOW(), updated_at = NOW() WHERE id = $1",
            "paused" => "UPDATE qd_chat_bots SET status = 'paused', updated_at = NOW() WHERE id = $1",
            _ => "UPDATE qd_chat_bots SET status = $2, updated_at = NOW() WHERE id = $1",
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
        sqlx::query("UPDATE qd_chat_bots SET last_decided_at = NOW() WHERE id = $1")
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
            r#"UPDATE qd_chat_bots SET
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
            r#"UPDATE qd_chat_bots SET
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
            r#"UPDATE qd_chat_bots SET
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
            r#"INSERT INTO pe_chat_order_context
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

    /* 平仓交易记录：先标记开仓记录为closed，再插入关联的平仓记录（通过paired_client_order_id关联） */
    async fn close_trade(
        &self,
        open_client_order_id: &str,
        close_client_order_id: &str,
        close_reason: &str,
    ) -> VirsResult<()> {
        let result = sqlx::query(
            r#"UPDATE pe_chat_order_context SET status = 'closed'
               WHERE client_order_id = $1 AND order_role = 'open' AND status = 'open'"#,
        )
        .bind(open_client_order_id)
        .execute(&self.db)
        .await?;

        if result.rows_affected() == 0 {
            warn!(open_client_order_id = %open_client_order_id, "No open trade found for close");
        }

        sqlx::query(
            r#"INSERT INTO pe_chat_order_context
               (client_order_id, bot_id, user_id, symbol, exchange, order_role, status, paired_client_order_id, close_reason, strategy_file)
               SELECT $1, bot_id, user_id, symbol, exchange, 'close', 'closed', client_order_id, $2, strategy_file
               FROM pe_chat_order_context
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
               FROM pe_chat_order_context
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
            r#"UPDATE pe_chat_order_context SET status = 'orphaned'
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
               FROM pe_chat_order_context close_ctx
               JOIN pe_chat_order_context open_ctx ON open_ctx.client_order_id = close_ctx.paired_client_order_id
               JOIN pe_order_latest open_ord ON open_ord.client_order_id = open_ctx.client_order_id
               WHERE close_ctx.bot_id = $1 AND close_ctx.order_role = 'close' AND close_ctx.status = 'closed'
               ORDER BY close_ctx.created_at DESC LIMIT 1"#,
        )
        .bind(bot_id)
        .fetch_optional(&self.db)
        .await?;
        Ok(row)
    }

    async fn update_trade_stop_loss(
        &self,
        client_order_id: &str,
        stop_loss: f64,
    ) -> VirsResult<()> {
        sqlx::query(
            r#"UPDATE pe_chat_order_context SET stop_loss = $2
               WHERE client_order_id = $1 AND status = 'open'"#,
        )
        .bind(client_order_id)
        .bind(stop_loss)
        .execute(&self.db)
        .await?;
        Ok(())
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
            r#"INSERT INTO pe_chat_order_context
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
            r#"INSERT INTO qd_chat_analysis_logs (bot_id, analysis_type, system_prompt, user_prompt, status, result, error, llm_model, completed_at, strategy_file)
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
            r#"UPDATE qd_chat_analysis_logs SET
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

    /* 加载连续亏损次数：从最近20笔平仓记录中，从新到旧统计连续亏损笔数，遇到盈利即停止 */
    async fn load_consecutive_losses(&self, bot_id: Uuid) -> VirsResult<i32> {
        let pnl_rows: Vec<(f64,)> = sqlx::query_as(
            r#"SELECT close_ord.realized_pnl::float AS pnl
               FROM pe_chat_order_context close_ctx
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
        sqlx::query("DELETE FROM qd_chat_bots WHERE id = $1")
            .bind(bot_id)
            .execute(&self.db)
            .await?;
        Ok(())
    }
}
