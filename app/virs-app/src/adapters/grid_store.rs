use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tracing::warn;
use uuid::Uuid;
use virs_error::VirsResult;

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

pub fn bot_to_config(b: &GridBot) -> GridBotConfig {
    GridBotConfig {
        id: b.id,
        user_id: b.user_id,
        name: b.name.clone(),
        symbol: b.symbol.clone(),
        exchange: b.exchange.clone(),
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
        strategy_file: b.strategy_file.clone(),
    }
}

#[async_trait]
impl GridStore for PgGridStore {
    async fn load_running_bots(&self) -> VirsResult<Vec<GridBotConfig>> {
        let bots: Vec<GridBot> =
            sqlx::query_as("SELECT * FROM qd_grid_bots WHERE status = 'running'")
                .fetch_all(&self.db)
                .await?;
        Ok(bots.iter().map(bot_to_config).collect())
    }

    async fn load_bot(&self, bot_id: Uuid) -> VirsResult<Option<GridBotConfig>> {
        let bot: Option<GridBot> = sqlx::query_as("SELECT * FROM qd_grid_bots WHERE id = $1")
            .bind(bot_id)
            .fetch_optional(&self.db)
            .await?;
        Ok(bot.as_ref().map(bot_to_config))
    }

    async fn load_trades(&self, bot_id: Uuid) -> VirsResult<Vec<GridTradeRecord>> {
        let rows: Vec<(
            String,
            Option<String>,
            i32,
            String,
            f64,
            f64,
            Option<String>,
            Option<f64>,
            Option<f64>,
            f64,
            DateTime<Utc>,
        )> = sqlx::query_as(
            r#"SELECT
                  open_ctx.client_order_id AS open_client_order_id,
                  close_ctx.client_order_id AS close_client_order_id,
                  open_ctx.grid_level,
                  LOWER(open_ord.side) AS open_side,
                  open_ord.avg_fill_price::float AS open_price,
                  open_ord.filled_qty::float AS open_quantity,
                  CASE WHEN close_ord.side IS NOT NULL THEN LOWER(close_ord.side) END AS close_side,
                  close_ord.avg_fill_price::float AS close_price,
                  close_ord.filled_qty::float AS close_quantity,
                  COALESCE(close_ord.realized_pnl::float, 0) AS pnl,
                  open_ctx.created_at AS opened_at
               FROM pe_grid_order_context open_ctx
               JOIN pe_order_latest open_ord ON open_ord.client_order_id = open_ctx.client_order_id
               LEFT JOIN pe_grid_order_context close_ctx ON close_ctx.paired_client_order_id = open_ctx.client_order_id
               LEFT JOIN pe_order_latest close_ord ON close_ord.client_order_id = close_ctx.client_order_id
               WHERE open_ctx.bot_id = $1 AND open_ctx.order_role = 'open'
               ORDER BY open_ctx.created_at ASC"#,
        )
        .bind(bot_id)
        .fetch_all(&self.db)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(
                    open_client_order_id,
                    close_client_order_id,
                    grid_level,
                    open_side,
                    open_price,
                    open_quantity,
                    close_side,
                    close_price,
                    close_quantity,
                    pnl,
                    opened_at,
                )| GridTradeRecord {
                    open_client_order_id,
                    close_client_order_id,
                    grid_level,
                    open_side,
                    open_price,
                    open_quantity,
                    close_side,
                    close_price,
                    close_quantity,
                    pnl,
                    opened_at,
                },
            )
            .collect())
    }

    async fn record_open_trade(
        &self,
        bot_id: Uuid,
        user_id: Uuid,
        symbol: &str,
        exchange: &str,
        grid_level: i32,
        client_order_id: &str,
        strategy_file: &Option<String>,
    ) -> VirsResult<()> {
        sqlx::query(
            r#"INSERT INTO pe_grid_order_context
               (client_order_id, bot_id, user_id, symbol, exchange, grid_level, order_role, status, strategy_file)
               VALUES ($1, $2, $3, $4, $5, $6, 'open', 'open', $7)"#,
        )
        .bind(client_order_id)
        .bind(bot_id)
        .bind(user_id)
        .bind(symbol)
        .bind(exchange)
        .bind(grid_level)
        .bind(strategy_file)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn close_trade(
        &self,
        open_client_order_id: &str,
        close_client_order_id: &str,
    ) -> VirsResult<()> {
        let result = sqlx::query(
            r#"UPDATE pe_grid_order_context SET status = 'closed'
               WHERE client_order_id = $1 AND order_role = 'open' AND status = 'open'"#,
        )
        .bind(open_client_order_id)
        .execute(&self.db)
        .await?;

        if result.rows_affected() == 0 {
            warn!(open_client_order_id = %open_client_order_id, "close_trade: no open trade found");
        }

        sqlx::query(
            r#"INSERT INTO pe_grid_order_context
               (client_order_id, bot_id, user_id, symbol, exchange, grid_level, order_role, status, paired_client_order_id, strategy_file)
               SELECT $1, bot_id, user_id, symbol, exchange, grid_level, 'close', 'closed', client_order_id, strategy_file
               FROM pe_grid_order_context
               WHERE client_order_id = $2 AND order_role = 'open'"#,
        )
        .bind(close_client_order_id)
        .bind(open_client_order_id)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    async fn find_open_trade(&self, bot_id: Uuid, grid_level: i32) -> VirsResult<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as(
            r#"SELECT client_order_id FROM pe_grid_order_context
               WHERE bot_id = $1 AND grid_level = $2 AND order_role = 'open' AND status = 'open'
               ORDER BY created_at DESC LIMIT 1"#,
        )
        .bind(bot_id)
        .bind(grid_level)
        .fetch_optional(&self.db)
        .await?;
        Ok(row.map(|r| r.0))
    }

    async fn record_orphaned_close_trade(
        &self,
        bot_id: Uuid,
        user_id: Uuid,
        symbol: &str,
        exchange: &str,
        grid_level: i32,
        close_client_order_id: &str,
        strategy_file: &Option<String>,
    ) -> VirsResult<()> {
        sqlx::query(
            r#"INSERT INTO pe_grid_order_context
               (client_order_id, bot_id, user_id, symbol, exchange, grid_level, order_role, status, strategy_file)
               VALUES ($1, $2, $3, $4, $5, $6, 'close', 'orphaned', $7)"#,
        )
        .bind(close_client_order_id)
        .bind(bot_id)
        .bind(user_id)
        .bind(symbol)
        .bind(exchange)
        .bind(grid_level)
        .bind(strategy_file)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn save_stats(
        &self,
        bot_id: Uuid,
        total_pnl: f64,
        unrealized_pnl: f64,
        total_trades: i32,
        grid_filled_count: i32,
        levels_json: Option<&serde_json::Value>,
    ) -> VirsResult<()> {
        sqlx::query(
            "UPDATE qd_grid_bots SET total_pnl = $2, unrealized_pnl = $3, total_trades = $4, grid_filled_count = $5, grid_levels_json = $6::jsonb, updated_at = NOW() WHERE id = $1",
        )
        .bind(bot_id).bind(total_pnl).bind(unrealized_pnl).bind(total_trades)
        .bind(grid_filled_count).bind(levels_json)
        .execute(&self.db).await?;
        Ok(())
    }

    async fn update_bot_status(&self, bot_id: Uuid, status: &str) -> VirsResult<()> {
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

    async fn update_last_adjusted(&self, bot_id: Uuid) -> VirsResult<()> {
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
    ) -> VirsResult<()> {
        sqlx::query("UPDATE qd_grid_bots SET upper_price = $2, lower_price = $3, updated_at = NOW() WHERE id = $1")
            .bind(bot_id).bind(upper_price).bind(lower_price)
            .execute(&self.db).await?;
        Ok(())
    }

    async fn update_quantity_per_grid(&self, bot_id: Uuid, quantity: f64) -> VirsResult<()> {
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
    ) -> VirsResult<()> {
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
        strategy_file: &Option<String>,
    ) -> VirsResult<()> {
        let status = if error.is_some() {
            "failed"
        } else {
            "completed"
        };
        sqlx::query(
            r#"INSERT INTO qd_grid_analysis_logs (bot_id, analysis_type, system_prompt, user_prompt, status, result, error, llm_model, completed_at, strategy_file)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW(), $9)"#,
        )
        .bind(bot_id).bind(analysis_type).bind(system_prompt).bind(user_prompt)
        .bind(status).bind(result).bind(error).bind(llm_model)
        .bind(strategy_file)
        .execute(&self.db).await?;
        Ok(())
    }

    async fn delete_bot(&self, bot_id: Uuid) -> VirsResult<()> {
        sqlx::query("DELETE FROM qd_grid_bots WHERE id = $1")
            .bind(bot_id)
            .execute(&self.db)
            .await?;
        Ok(())
    }
}
