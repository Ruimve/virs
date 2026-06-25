//! PgAutoStore — PostgreSQL implementation of AutoStore.

use async_trait::async_trait;
use sqlx::PgPool;
use tracing::warn;
use uuid::Uuid;

use virs_types::auto_port::*;
use virs_types::auto_port::AutoBotConfig;
use virs_types::auto_port::AutoMarketType;
use virs_models::AutoBot;

pub struct PgAutoStore {
    db: PgPool,
}

impl PgAutoStore {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }
}

fn bot_to_config(bot: &AutoBot) -> AutoBotConfig {
    AutoBotConfig {
        id: bot.id,
        user_id: bot.user_id,
        name: bot.name.clone(),
        symbol: bot.symbol.clone(),
        exchange: bot.exchange.clone(),
        market_type: AutoMarketType::from_str_lossy(&bot.market_type),
        paper_mode: bot.paper_mode,
        leverage: bot.leverage,
        max_position_pct: bot.max_position_pct,
        decide_interval_secs: bot.decide_interval_secs,
        position_id: bot.position_id,
        market_regime: bot.market_regime.clone(),
        ai_analysis: bot.ai_analysis.clone(),
        system_prompt: bot.system_prompt.clone(),
        user_prompt: bot.user_prompt.clone(),
        total_pnl: bot.total_pnl,
        total_trades: bot.total_trades,
        win_trades: bot.win_trades,
        loss_trades: bot.loss_trades,
        last_decided_at: bot.last_decided_at,
    }
}

#[async_trait]
impl AutoStore for PgAutoStore {
    async fn load_running_bots(&self) -> anyhow::Result<Vec<AutoBotConfig>> {
        let bots: Vec<AutoBot> =
            sqlx::query_as("SELECT * FROM qd_auto_bots WHERE status = 'running'")
                .fetch_all(&self.db).await?;
        Ok(bots.iter().map(bot_to_config).collect())
    }

    async fn load_bot(&self, bot_id: Uuid) -> anyhow::Result<Option<AutoBotConfig>> {
        let bot: Option<AutoBot> =
            sqlx::query_as("SELECT * FROM qd_auto_bots WHERE id = $1")
                .bind(bot_id).fetch_optional(&self.db).await?;
        Ok(bot.as_ref().map(bot_to_config))
    }

    async fn update_bot_status(&self, bot_id: Uuid, status: &str) -> anyhow::Result<()> {
        let sql = match status {
            "running" => "UPDATE qd_auto_bots SET status = 'running', started_at = NOW(), updated_at = NOW() WHERE id = $1",
            "stopped" => "UPDATE qd_auto_bots SET status = 'stopped', stopped_at = NOW(), updated_at = NOW() WHERE id = $1",
            "paused" => "UPDATE qd_auto_bots SET status = 'paused', updated_at = NOW() WHERE id = $1",
            _ => "UPDATE qd_auto_bots SET status = $2, updated_at = NOW() WHERE id = $1",
        };
        if status == "running" || status == "stopped" || status == "paused" {
            sqlx::query(sql).bind(bot_id).execute(&self.db).await?;
        } else {
            sqlx::query(sql).bind(bot_id).bind(status).execute(&self.db).await?;
        }
        Ok(())
    }

    async fn update_last_decided(&self, bot_id: Uuid) -> anyhow::Result<()> {
        sqlx::query("UPDATE qd_auto_bots SET last_decided_at = NOW() WHERE id = $1")
            .bind(bot_id).execute(&self.db).await?;
        Ok(())
    }

    async fn update_position(
        &self, bot_id: Uuid, position_id: Option<Uuid>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"UPDATE qd_auto_bots SET
                position_id = $2, updated_at = NOW()
               WHERE id = $1"#,
        )
        .bind(bot_id).bind(position_id)
        .execute(&self.db).await?;
        Ok(())
    }

    async fn update_ai_analysis(
        &self, bot_id: Uuid, market_regime: &str, leverage: i32, ai_analysis: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"UPDATE qd_auto_bots SET
                market_regime = $2, leverage = $3, ai_analysis = $4, updated_at = NOW()
               WHERE id = $1"#,
        )
        .bind(bot_id).bind(market_regime).bind(leverage).bind(ai_analysis)
        .execute(&self.db).await?;
        Ok(())
    }

    async fn update_stats(
        &self, bot_id: Uuid, total_pnl: f64, total_trades: i32, win_trades: i32, loss_trades: i32,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"UPDATE qd_auto_bots SET
                total_pnl = $2, total_trades = $3, win_trades = $4, loss_trades = $5, updated_at = NOW()
               WHERE id = $1"#,
        )
        .bind(bot_id).bind(total_pnl).bind(total_trades).bind(win_trades).bind(loss_trades)
        .execute(&self.db).await?;
        Ok(())
    }

    async fn record_open_trade(
        &self, bot_id: Uuid, user_id: Uuid, symbol: &str, exchange: &str,
        open_side: &str, open_price: f64, open_quantity: f64,
        open_fee: f64, open_order_id: Option<&str>,
        stop_loss: f64, take_profit: f64,
    ) -> anyhow::Result<Uuid> {
        let row: (Uuid,) = sqlx::query_as(
            r#"INSERT INTO qd_auto_trades
               (bot_id, user_id, symbol, exchange, open_side, open_price, open_quantity,
                open_order_id, open_fee, stop_loss, take_profit,
                trigger_source, status)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, 'llm', 'open')
               RETURNING id"#,
        )
        .bind(bot_id).bind(user_id).bind(symbol).bind(exchange)
        .bind(open_side).bind(open_price).bind(open_quantity)
        .bind(open_order_id).bind(open_fee)
        .bind(stop_loss).bind(take_profit)
        .fetch_one(&self.db).await?;
        Ok(row.0)
    }

    async fn close_trade(
        &self, trade_id: Uuid, close_side: &str, close_price: f64,
        close_quantity: f64, close_order_id: Option<&str>,
        close_fee: f64, pnl: f64, pnl_pct: f64,
        trigger_source: &str, close_reason: &str,
    ) -> anyhow::Result<()> {
        let pnl_pct = if pnl_pct.is_nan() { 0.0 } else { pnl_pct };
        let result = sqlx::query(
            r#"UPDATE qd_auto_trades SET
               close_side = $2, close_price = $3, close_quantity = $4,
               close_order_id = $5, close_fee = $6, closed_at = NOW(),
               pnl = $7, pnl_pct = $8,
               trigger_source = $9, close_reason = $10,
               status = 'closed'
               WHERE id = $1 AND status = 'open'"#,
        )
        .bind(trade_id).bind(close_side).bind(close_price).bind(close_quantity)
        .bind(close_order_id).bind(close_fee).bind(pnl).bind(pnl_pct)
        .bind(trigger_source).bind(close_reason)
        .execute(&self.db).await?;

        if result.rows_affected() == 0 {
            warn!(trade_id = %trade_id, "close_trade: no open trade found");
        }
        Ok(())
    }

    async fn find_open_trade(&self, bot_id: Uuid) -> anyhow::Result<Option<(Uuid, f64, f64)>> {
        let row: Option<(Uuid, f64, f64)> = sqlx::query_as(
            "SELECT id, stop_loss, take_profit FROM qd_auto_trades WHERE bot_id = $1 AND status = 'open' ORDER BY opened_at DESC LIMIT 1",
        )
        .bind(bot_id)
        .fetch_optional(&self.db).await?;
        Ok(row)
    }

    async fn update_trade_stop_loss(
        &self, trade_id: Uuid, stop_loss: f64,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"UPDATE qd_auto_trades SET stop_loss = $2 WHERE id = $1 AND status = 'open'"#,
        )
        .bind(trade_id).bind(stop_loss)
        .execute(&self.db).await?;
        Ok(())
    }
    async fn record_orphaned_close_trade(
        &self, bot_id: Uuid, user_id: Uuid, symbol: &str, exchange: &str,
        close_side: &str, close_price: f64, close_quantity: f64,
        close_order_id: Option<&str>, close_fee: f64,
        pnl: f64, pnl_pct: f64, trigger_source: &str, close_reason: &str,
    ) -> anyhow::Result<Uuid> {
        let open_side = if close_side == "buy" { "sell" } else { "buy" };
        let pnl_pct = if pnl_pct.is_nan() { 0.0 } else { pnl_pct };
        let row: (Uuid,) = sqlx::query_as(
            r#"INSERT INTO qd_auto_trades
               (bot_id, user_id, symbol, exchange,
                open_side, open_price, open_quantity, open_fee,
                close_side, close_price, close_quantity, close_order_id, close_fee, closed_at,
                pnl, pnl_pct, trigger_source, close_reason, status)
               VALUES ($1, $2, $3, $4,
                       $5, 0, $6, 0,
                       $7, $8, $9, $10, $11, NOW(),
                       $12, $13, $14, $15, 'orphaned')
               RETURNING id"#,
        )
        .bind(bot_id).bind(user_id).bind(symbol).bind(exchange)
        .bind(open_side).bind(close_quantity)
        .bind(close_side).bind(close_price).bind(close_quantity).bind(close_order_id)
        .bind(close_fee).bind(pnl).bind(pnl_pct).bind(trigger_source).bind(close_reason)
        .fetch_one(&self.db).await?;
        Ok(row.0)
    }

    async fn save_analysis_log(
        &self, bot_id: Uuid, analysis_type: &str, system_prompt: &str,
        user_prompt: &str, result: &serde_json::Value, error: Option<&str>,
        llm_model: &str,
    ) -> anyhow::Result<()> {
        let status = if error.is_some() { "failed" } else { "completed" };
        sqlx::query(
            r#"INSERT INTO qd_auto_analysis_logs (bot_id, analysis_type, system_prompt, user_prompt, status, result, error, llm_model, completed_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())"#,
        )
        .bind(bot_id).bind(analysis_type).bind(system_prompt).bind(user_prompt)
        .bind(status).bind(result).bind(error).bind(llm_model)
        .execute(&self.db).await?;
        Ok(())
    }

    async fn load_analysis_logs(&self, bot_id: Uuid) -> anyhow::Result<Vec<AutoAnalysisLogEntry>> {
        let rows: Vec<(Uuid, Uuid, String, String, String, serde_json::Value, Option<String>, String, chrono::DateTime<chrono::Utc>)> =
            sqlx::query_as(
                r#"SELECT id, bot_id, analysis_type, system_prompt, user_prompt, result, error, llm_model, created_at
                   FROM qd_auto_analysis_logs WHERE bot_id = $1 ORDER BY created_at DESC LIMIT 50"#,
            )
            .bind(bot_id).fetch_all(&self.db).await?;

        Ok(rows.into_iter().map(|r| AutoAnalysisLogEntry {
            id: r.0, bot_id: r.1, analysis_type: r.2,
            system_prompt: r.3, user_prompt: r.4, result: r.5,
            error: r.6, llm_model: r.7, created_at: r.8,
        }).collect())
    }

    async fn load_consecutive_losses(&self, bot_id: Uuid) -> anyhow::Result<i32> {
        let pnl_rows: Vec<(f64,)> = sqlx::query_as(
            r#"SELECT pnl FROM qd_auto_trades
               WHERE bot_id = $1 AND status = 'closed'
               ORDER BY closed_at DESC LIMIT 20"#
        )
        .bind(bot_id).fetch_all(&self.db).await?;

        let mut count = 0i32;
        for (pnl,) in &pnl_rows {
            if *pnl < 0.0 { count += 1; } else { break; }
        }
        Ok(count)
    }

    async fn delete_bot(&self, bot_id: Uuid) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM qd_auto_bots WHERE id = $1")
            .bind(bot_id).execute(&self.db).await?;
        Ok(())
    }
}
