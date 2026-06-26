//! PgGridStore — PostgreSQL implementation of GridStore.

use async_trait::async_trait;
use sqlx::PgPool;
use tracing::warn;
use uuid::Uuid;

use virs_models::GridBot;
use virs_types::grid_port::*;

pub struct PgGridStore {
    db: PgPool,
}

impl PgGridStore {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }
}

fn bot_to_config(b: &GridBot) -> GridBotConfig {
    GridBotConfig {
        id: b.id,
        user_id: b.user_id,
        name: b.name.clone(),
        symbol: b.symbol.clone(),
        exchange: b.exchange.clone(),
        market_type: b.market_type.clone(),
        paper_mode: b.paper_mode,
        grid_count: b.grid_count,
        upper_price: b.upper_price,
        lower_price: b.lower_price,
        grid_profit_pct: b.grid_profit_pct,
        quantity_per_grid: b.quantity_per_grid,
        leverage: b.leverage,
        dynamic_adjust: b.dynamic_adjust,
        adjust_interval_secs: b.adjust_interval_secs,
        market_regime: b.market_regime.clone(),
        grid_levels_json: b.grid_levels_json.clone(),
        system_prompt: b.system_prompt.clone(),
        last_adjusted_at: b.last_adjusted_at,
    }
}

#[async_trait]
impl GridStore for PgGridStore {
    async fn load_running_bots(&self) -> anyhow::Result<Vec<GridBotConfig>> {
        let bots: Vec<GridBot> =
            sqlx::query_as("SELECT * FROM qd_grid_bots WHERE status = 'running'")
                .fetch_all(&self.db)
                .await?;
        Ok(bots.iter().map(bot_to_config).collect())
    }

    async fn load_bot(&self, bot_id: Uuid) -> anyhow::Result<Option<GridBotConfig>> {
        let bot: Option<GridBot> = sqlx::query_as("SELECT * FROM qd_grid_bots WHERE id = $1")
            .bind(bot_id)
            .fetch_optional(&self.db)
            .await?;
        Ok(bot.as_ref().map(bot_to_config))
    }

    async fn load_trades(&self, bot_id: Uuid) -> anyhow::Result<Vec<GridTradeRecord>> {
        let trades: Vec<virs_models::GridTrade> =
            sqlx::query_as("SELECT * FROM qd_grid_trades WHERE bot_id = $1 ORDER BY opened_at ASC")
                .bind(bot_id)
                .fetch_all(&self.db)
                .await?;

        Ok(trades
            .into_iter()
            .map(|t| GridTradeRecord {
                id: t.id,
                grid_level: t.grid_level,
                open_side: t.open_side,
                open_price: t.open_price,
                open_quantity: t.open_quantity,
                close_side: t.close_side,
                close_price: t.close_price,
                close_quantity: t.close_quantity,
                pnl: t.pnl,
                opened_at: t.opened_at,
            })
            .collect())
    }

    async fn record_open_trade(
        &self,
        bot_id: Uuid,
        user_id: Uuid,
        symbol: &str,
        exchange: &str,
        grid_level: i32,
        open_side: &str,
        open_price: f64,
        open_quantity: f64,
        open_order_id: Option<&str>,
    ) -> anyhow::Result<Uuid> {
        let row: (Uuid,) = sqlx::query_as(
            r#"INSERT INTO qd_grid_trades (bot_id, user_id, symbol, exchange, grid_level, open_side, open_price, open_quantity, open_order_id, status)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'open')
               RETURNING id"#,
        )
        .bind(bot_id).bind(user_id).bind(symbol).bind(exchange)
        .bind(grid_level).bind(open_side).bind(open_price).bind(open_quantity)
        .bind(open_order_id)
        .fetch_one(&self.db).await?;
        Ok(row.0)
    }

    async fn close_trade(
        &self,
        trade_id: Uuid,
        close_side: &str,
        close_price: f64,
        close_quantity: f64,
        close_order_id: Option<&str>,
        pnl: f64,
        pnl_pct: f64,
    ) -> anyhow::Result<()> {
        let pnl_pct = if pnl_pct.is_nan() { 0.0 } else { pnl_pct };
        let result = sqlx::query(
            r#"UPDATE qd_grid_trades SET
               close_side = $2, close_price = $3, close_quantity = $4,
               close_order_id = $5, closed_at = NOW(),
               pnl = $6, pnl_pct = $7, status = 'closed'
               WHERE id = $1 AND status = 'open'"#,
        )
        .bind(trade_id)
        .bind(close_side)
        .bind(close_price)
        .bind(close_quantity)
        .bind(close_order_id)
        .bind(pnl)
        .bind(pnl_pct)
        .execute(&self.db)
        .await?;

        if result.rows_affected() == 0 {
            warn!(trade_id = %trade_id, "close_trade: no open trade found");
        }
        Ok(())
    }

    async fn find_open_trade(&self, bot_id: Uuid, grid_level: i32) -> anyhow::Result<Option<Uuid>> {
        let row: Option<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM qd_grid_trades WHERE bot_id = $1 AND grid_level = $2 AND status = 'open' ORDER BY opened_at DESC LIMIT 1",
        )
        .bind(bot_id).bind(grid_level)
        .fetch_optional(&self.db).await?;
        Ok(row.map(|r| r.0))
    }

    async fn record_orphaned_close_trade(
        &self,
        bot_id: Uuid,
        user_id: Uuid,
        symbol: &str,
        exchange: &str,
        grid_level: i32,
        close_side: &str,
        close_price: f64,
        close_quantity: f64,
        close_order_id: Option<&str>,
        pnl: f64,
        pnl_pct: f64,
    ) -> anyhow::Result<Uuid> {
        let open_side = if close_side == "buy" { "sell" } else { "buy" };
        let pnl_pct = if pnl_pct.is_nan() { 0.0 } else { pnl_pct };
        let row: (Uuid,) = sqlx::query_as(
            r#"INSERT INTO qd_grid_trades (bot_id, user_id, symbol, exchange, grid_level, open_side, open_price, open_quantity, close_side, close_price, close_quantity, close_order_id, closed_at, pnl, pnl_pct, status)
               VALUES ($1, $2, $3, $4, $5, $6, 0, $7, $8, $9, $10, $11, NOW(), $12, $13, 'orphaned')
               RETURNING id"#,
        )
        .bind(bot_id).bind(user_id).bind(symbol).bind(exchange).bind(grid_level)
        .bind(open_side).bind(close_quantity).bind(close_side).bind(close_price)
        .bind(close_quantity).bind(close_order_id).bind(pnl).bind(pnl_pct)
        .fetch_one(&self.db).await?;
        Ok(row.0)
    }

    async fn save_stats(
        &self,
        bot_id: Uuid,
        total_pnl: f64,
        unrealized_pnl: f64,
        total_trades: i32,
        grid_filled_count: i32,
        levels_json: Option<&serde_json::Value>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE qd_grid_bots SET total_pnl = $2, unrealized_pnl = $3, total_trades = $4, grid_filled_count = $5, grid_levels_json = $6::jsonb, updated_at = NOW() WHERE id = $1",
        )
        .bind(bot_id).bind(total_pnl).bind(unrealized_pnl).bind(total_trades)
        .bind(grid_filled_count).bind(levels_json)
        .execute(&self.db).await?;
        Ok(())
    }

    async fn update_bot_status(&self, bot_id: Uuid, status: &str) -> anyhow::Result<()> {
        let sql = match status {
            "running" => "UPDATE qd_grid_bots SET status = 'running', started_at = NOW(), updated_at = NOW() WHERE id = $1",
            "stopped" => "UPDATE qd_grid_bots SET status = 'stopped', stopped_at = NOW(), updated_at = NOW() WHERE id = $1",
            "paused" => "UPDATE qd_grid_bots SET status = 'paused', updated_at = NOW() WHERE id = $1",
            _ => "UPDATE qd_grid_bots SET status = $2, updated_at = NOW() WHERE id = $1",
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

    async fn update_last_adjusted(&self, bot_id: Uuid) -> anyhow::Result<()> {
        sqlx::query("UPDATE qd_grid_bots SET last_adjusted_at = NOW() WHERE id = $1")
            .bind(bot_id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    async fn update_grid_params(
        &self,
        bot_id: Uuid,
        upper_price: f64,
        lower_price: f64,
    ) -> anyhow::Result<()> {
        sqlx::query("UPDATE qd_grid_bots SET upper_price = $2, lower_price = $3, updated_at = NOW() WHERE id = $1")
            .bind(bot_id).bind(upper_price).bind(lower_price)
            .execute(&self.db).await?;
        Ok(())
    }

    async fn update_quantity_per_grid(&self, bot_id: Uuid, quantity: f64) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE qd_grid_bots SET quantity_per_grid = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(bot_id)
        .bind(quantity)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn update_ai_analysis(
        &self,
        bot_id: Uuid,
        market_regime: &str,
        upper_price: f64,
        lower_price: f64,
        grid_count: i32,
        grid_profit_pct: f64,
        quantity_per_grid: f64,
        leverage: i32,
        ai_analysis: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"UPDATE qd_grid_bots SET
                market_regime = $1, upper_price = $2, lower_price = $3,
                grid_count = $4, grid_profit_pct = $5, quantity_per_grid = $6,
                leverage = $7, ai_analysis = $8,
                last_adjusted_at = NOW(), updated_at = NOW()
               WHERE id = $9"#,
        )
        .bind(market_regime)
        .bind(upper_price)
        .bind(lower_price)
        .bind(grid_count)
        .bind(grid_profit_pct)
        .bind(quantity_per_grid)
        .bind(leverage)
        .bind(ai_analysis)
        .bind(bot_id)
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
    ) -> anyhow::Result<()> {
        let status = if error.is_some() {
            "failed"
        } else {
            "completed"
        };
        sqlx::query(
            r#"INSERT INTO qd_grid_analysis_logs (bot_id, analysis_type, system_prompt, user_prompt, status, result, error, llm_model, completed_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())"#,
        )
        .bind(bot_id).bind(analysis_type).bind(system_prompt).bind(user_prompt)
        .bind(status).bind(result).bind(error).bind(llm_model)
        .execute(&self.db).await?;
        Ok(())
    }

    async fn load_analysis_logs(&self, bot_id: Uuid) -> anyhow::Result<Vec<AnalysisLogEntry>> {
        let rows: Vec<(Uuid, Uuid, String, String, String, serde_json::Value, Option<String>, String, chrono::DateTime<chrono::Utc>)> =
            sqlx::query_as(
                r#"SELECT id, bot_id, analysis_type, system_prompt, user_prompt, result, error, llm_model, created_at
                   FROM qd_grid_analysis_logs WHERE bot_id = $1 ORDER BY created_at DESC LIMIT 50"#,
            )
            .bind(bot_id)
            .fetch_all(&self.db).await?;

        Ok(rows
            .into_iter()
            .map(|r| AnalysisLogEntry {
                id: r.0,
                bot_id: r.1,
                analysis_type: r.2,
                system_prompt: r.3,
                user_prompt: r.4,
                result: r.5,
                error: r.6,
                llm_model: r.7,
                created_at: r.8,
            })
            .collect())
    }

    async fn delete_bot(&self, bot_id: Uuid) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM qd_grid_bots WHERE id = $1")
            .bind(bot_id)
            .execute(&self.db)
            .await?;
        Ok(())
    }
}
