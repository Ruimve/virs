/*! Grid Engine 的 Adapter 实现 */
/*!  */
/*! 将外部模块（ExchangeRegistry, Position Engine, Database, AiService 等） */
/*! 适配为 semi_automatic_grid 模块定义的 trait 接口。 */

use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::bot::semi_automatic_grid::ports::*;
use crate::config::AiConfig;
use crate::trading::exchange::registry::ExchangeRegistry;
use crate::engine::kline::KlineEngine;
use crate::engine::kline::types::Timeframe;
use crate::models::Kline;

// Re-export from common
pub use crate::bot::common::adapters::{
    candle_to_kline, convert_pe_event, DefaultLlmResolver, ExchangePriceProvider as CommonExchangePriceProvider,
    PeOrderExecutor, PgCredentialStore, SwitchableOrderExecutor,
};

pub struct ExchangePriceProvider {
    inner: CommonExchangePriceProvider,
}

impl ExchangePriceProvider {
    pub fn new(exchange_registry: Arc<ExchangeRegistry>) -> Self {
        Self { inner: CommonExchangePriceProvider::new(exchange_registry, "perpetual") }
    }

    pub fn with_kline_engine(mut self, engine: Arc<KlineEngine>) -> Self {
        self.inner = self.inner.with_kline_engine(engine);
        self
    }
}

#[async_trait]
impl PriceProvider for ExchangePriceProvider {
    async fn get_price(&self, exchange: &str, symbol: &str) -> Option<f64> {
        self.inner.get_price(exchange, symbol).await
    }
}

// ── MarketDataProvider ──

pub struct ExchangeMarketDataProvider {
    exchange_registry: Arc<ExchangeRegistry>,
    kline_engine: Option<Arc<KlineEngine>>,
}

impl ExchangeMarketDataProvider {
    pub fn new(exchange_registry: Arc<ExchangeRegistry>) -> Self {
        Self { exchange_registry, kline_engine: None }
    }

    pub fn with_kline_engine(mut self, engine: Arc<KlineEngine>) -> Self {
        self.kline_engine = Some(engine);
        self
    }

/** 从缓存或 REST API 获取 K 线数据

优先从 KlineEngine 缓存获取，缓存不足时回退到交易所 REST API。
required 为 true 时，数据不足返回 None；为 false 时返回空向量 */
    async fn fetch_klines(
        &self,
        exchange: &str,
        symbol: &str,
        timeframe: Timeframe,
        min_count: usize,
        interval_str: &str,
        start_ms: i64,
        required: bool,
    ) -> Option<Vec<Kline>> {
        if let Some(klines) = self.get_klines_from_cache_or_rest(exchange, symbol, timeframe, min_count).await {
            tracing::info!(exchange, symbol, interval = interval_str, count = klines.len(), source = "cache", "Fetched klines");
            return Some(klines);
        }

        let exchange_key = format!("{}:perpetual", exchange);
        let ex = self.exchange_registry.get(&exchange_key);
        let result = match ex {
            Some(ref ex) => match ex.get_klines_range(symbol, interval_str, start_ms, chrono::Utc::now().timestamp_millis()).await {
                Ok(k) if k.len() >= min_count => Some(k),
                Ok(k) if !required => Some(k),
                Ok(k) => {
                    warn!(exchange, symbol, count = k.len(), required = min_count, "{} klines insufficient, returning empty", interval_str);
                    if required { None } else { Some(k) }
                }
                Err(e) => {
                    warn!(exchange, symbol, error = %e, "Failed to fetch {} klines", interval_str);
                    if required { None } else { Some(vec![]) }
                }
            },
            None => {
                warn!(exchange, symbol, "No {} klines in cache and no exchange for REST fallback", interval_str);
                if required { None } else { Some(vec![]) }
            }
        };
        if let Some(ref klines) = result {
            tracing::info!(exchange, symbol, interval = interval_str, count = klines.len(), source = "rest", "Fetched klines");
        }
        result
    }

/** 从缓存获取 K 线数据（内部辅助方法） */
    async fn get_klines_from_cache_or_rest(
        &self,
        exchange: &str,
        symbol: &str,
        timeframe: Timeframe,
        min_count: usize,
    ) -> Option<Vec<Kline>> {
        if let Some(ref engine) = self.kline_engine {
            if let Some(candles) = engine.get_klines_async(exchange, symbol, timeframe).await {
                if candles.len() >= min_count {
                    return Some(candles.iter().map(candle_to_kline).collect());
                }
                debug!(
                    exchange, symbol, timeframe = timeframe.as_str(),
                    cached = candles.len(), required = min_count,
                    "KlineEngine cache insufficient, falling back to REST"
                );
            }
        }
        None
    }

/** 获取当前价格

优先从 1 分钟 K 线缓存获取，回退到交易所 ticker，最后回退到 1h K 线收盘价 */
    async fn fetch_current_price(&self, exchange: &str, symbol: &str, klines_1h: &[Kline]) -> f64 {
        if let Some(ref engine) = self.kline_engine {
            if let Some(candles) = engine.get_klines_async(exchange, symbol, Timeframe::M1).await {
                if let Some(last) = candles.last() {
                    if last.close > 0.0 {
                        return last.close;
                    }
                }
            }
        }

        let exchange_key = format!("{}:perpetual", exchange);
        if let Some(ex) = self.exchange_registry.get(&exchange_key) {
            if let Ok(t) = ex.get_ticker(symbol).await {
                if t.last > 0.0 {
                    return t.last;
                }
            }
        }

        klines_1h.last().map(|k| k.close).unwrap_or(0.0)
    }
}

#[async_trait]
impl MarketDataProvider for ExchangeMarketDataProvider {
    async fn get_market_snapshot(&self, exchange: &str, symbol: &str) -> MarketSnapshot {
        let now_ms = chrono::Utc::now().timestamp_millis();

        let klines_1h = match self.fetch_klines(
            exchange, symbol, Timeframe::H1, 30, "1h",
            now_ms - 200 * 3600 * 1000, true,
        ).await {
            Some(k) => k,
            None => return MarketSnapshot::default(),
        };

        let klines_4h = self.fetch_klines(
            exchange, symbol, Timeframe::H4, 50, "4h",
            now_ms - 100 * 4 * 3600 * 1000, false,
        ).await.unwrap_or_default();
        tracing::info!(exchange, symbol, h4_count = klines_4h.len(), "Fetched 4h klines for market snapshot");

        let klines_15m = self.fetch_klines(
            exchange, symbol, Timeframe::M15, 50, "15m",
            now_ms - 200 * 15 * 60 * 1000, false,
        ).await.unwrap_or_default();

        let current_price = self.fetch_current_price(exchange, symbol, &klines_1h).await;

        let exchange_key = format!("{}:perpetual", exchange);
        let funding_rate = if let Some(ex) = self.exchange_registry.get(&exchange_key) {
            ex.get_funding_rate(symbol).await.map(|fr| fr.rate).unwrap_or(0.0)
        } else {
            0.0
        };

        let ind = super::utils::compute_market_indicators(
            &klines_1h,
            &klines_4h,
            &klines_15m,
            funding_rate,
            "N/A".to_string(),
        );

        let effective_price = if current_price > 0.0 { current_price } else { ind.current_price };

        MarketSnapshot {
            current_price: effective_price,
            funding_rate,
            indicators: ind,
        }
    }

    async fn get_account_balance(&self, exchange: &str) -> AccountBalance {
        let exchange_key = format!("{}:perpetual", exchange);
        
        let ex = match self.exchange_registry.get(&exchange_key) {
            Some(e) => e,
            None => {
                tracing::warn!("[get_account_balance] Exchange NOT found in registry, returning default");
                return AccountBalance::default();
            },
        };
        
        match ex.get_balances().await {
            Ok(bs) => {
                let usdt = bs.iter().find(|b| b.asset.eq_ignore_ascii_case("USDT"));
                match usdt {
                    Some(b) => {
                        AccountBalance {
                            total: b.total,
                            free: b.free,
                            used: b.used,
                        }
                    },
                    None => {
                        tracing::warn!("[get_account_balance] No USDT balance found");
                        AccountBalance::default()
                    },
                }
            }
            Err(e) => {
                tracing::error!("[get_account_balance] get_balances error: {:?}", e);
                AccountBalance::default()
            },
        }
    }
}

// ── GridStore ──

pub struct PgGridStore {
    db: PgPool,
}

impl PgGridStore {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl GridStore for PgGridStore {
    async fn load_running_bots(&self) -> anyhow::Result<Vec<GridBotConfig>> {
        let bots: Vec<crate::models::GridBot> =
            sqlx::query_as("SELECT * FROM qd_grid_bots WHERE status = 'running'")
                .fetch_all(&self.db)
                .await?;

        Ok(bots.into_iter().map(|b| bot_to_config(&b)).collect())
    }

    async fn load_bot(&self, bot_id: Uuid) -> anyhow::Result<Option<GridBotConfig>> {
        let bot: Option<crate::models::GridBot> =
            sqlx::query_as("SELECT * FROM qd_grid_bots WHERE id = $1")
                .bind(bot_id)
                .fetch_optional(&self.db)
                .await?;

        Ok(bot.map(|b| bot_to_config(&b)))
    }

    async fn load_trades(&self, bot_id: Uuid) -> anyhow::Result<Vec<GridTradeRecord>> {
        let trades: Vec<crate::models::GridTrade> = sqlx::query_as(
            "SELECT * FROM qd_grid_trades WHERE bot_id = $1 ORDER BY opened_at ASC",
        )
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
        .bind(bot_id)
        .bind(user_id)
        .bind(symbol)
        .bind(exchange)
        .bind(grid_level)
        .bind(open_side)
        .bind(open_price)
        .bind(open_quantity)
        .bind(open_order_id)
        .fetch_one(&self.db)
        .await?;
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
            warn!(trade_id = %trade_id, "close_trade: no open trade found, may already be closed");
        }
        Ok(())
    }

    async fn find_open_trade(&self, bot_id: Uuid, grid_level: i32) -> anyhow::Result<Option<Uuid>> {
        let row: Option<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM qd_grid_trades WHERE bot_id = $1 AND grid_level = $2 AND status = 'open' ORDER BY opened_at DESC LIMIT 1",
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
        close_side: &str,
        close_price: f64,
        close_quantity: f64,
        close_order_id: Option<&str>,
        pnl: f64,
        pnl_pct: f64,
    ) -> anyhow::Result<Uuid> {
        let open_side = if close_side == "buy" { "sell" } else { "buy" };
        let pnl_pct = if pnl_pct.is_nan() { 0.0 } else { pnl_pct };
        let open_quantity = close_quantity;
        let row: (Uuid,) = sqlx::query_as(
            r#"INSERT INTO qd_grid_trades (bot_id, user_id, symbol, exchange, grid_level, open_side, open_price, open_quantity, close_side, close_price, close_quantity, close_order_id, closed_at, pnl, pnl_pct, status)
               VALUES ($1, $2, $3, $4, $5, $6, 0, $7, $8, $9, $10, $11, NOW(), $12, $13, 'orphaned')
               RETURNING id"#,
        )
        .bind(bot_id)
        .bind(user_id)
        .bind(symbol)
        .bind(exchange)
        .bind(grid_level)
        .bind(open_side)
        .bind(open_quantity)
        .bind(close_side)
        .bind(close_price)
        .bind(close_quantity)
        .bind(close_order_id)
        .bind(pnl)
        .bind(pnl_pct)
        .fetch_one(&self.db)
        .await?;
        Ok(row.0)
    }

    async fn save_stats(&self, bot_id: Uuid, total_pnl: f64, unrealized_pnl: f64, total_trades: i32, grid_filled_count: i32, levels_json: Option<&serde_json::Value>) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE qd_grid_bots SET total_pnl = $2, unrealized_pnl = $3, total_trades = $4, grid_filled_count = $5, grid_levels_json = $6::jsonb, updated_at = NOW() WHERE id = $1",
        )
        .bind(bot_id)
        .bind(total_pnl)
        .bind(unrealized_pnl)
        .bind(total_trades)
        .bind(grid_filled_count)
        .bind(levels_json)
        .execute(&self.db)
        .await?;
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
            sqlx::query(sql).bind(bot_id).bind(status).execute(&self.db).await?;
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

    async fn update_grid_params(&self, bot_id: Uuid, upper_price: f64, lower_price: f64) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE qd_grid_bots SET upper_price = $2, lower_price = $3, updated_at = NOW() WHERE id = $1",
        )
        .bind(bot_id)
        .bind(upper_price)
        .bind(lower_price)
        .execute(&self.db)
        .await?;
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
    ) -> anyhow::Result<()> {
        let status = if error.is_some() { "failed" } else { "completed" };
        sqlx::query(
            r#"INSERT INTO qd_grid_analysis_logs (bot_id, analysis_type, system_prompt, user_prompt, status, result, error, completed_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())"#,
        )
        .bind(bot_id)
        .bind(analysis_type)
        .bind(system_prompt)
        .bind(user_prompt)
        .bind(status)
        .bind(result)
        .bind(error)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn load_analysis_logs(&self, bot_id: Uuid) -> anyhow::Result<Vec<AnalysisLogEntry>> {
        let rows: Vec<(Uuid, Uuid, String, String, String, serde_json::Value, Option<String>, chrono::DateTime<chrono::Utc>)> =
            sqlx::query_as(
                r#"SELECT id, bot_id, analysis_type, system_prompt, user_prompt, result, error, created_at
                   FROM qd_grid_analysis_logs WHERE bot_id = $1 ORDER BY created_at DESC LIMIT 50"#,
            )
            .bind(bot_id)
            .fetch_all(&self.db)
            .await?;

        Ok(rows.into_iter().map(|r| AnalysisLogEntry {
            id: r.0, bot_id: r.1, analysis_type: r.2,
            system_prompt: r.3, user_prompt: r.4, result: r.5,
            error: r.6, created_at: r.7,
        }).collect())
    }

    async fn delete_bot(&self, bot_id: Uuid) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM qd_grid_bots WHERE id = $1")
            .bind(bot_id)
            .execute(&self.db)
            .await?;
        Ok(())
    }
}

fn bot_to_config(bot: &crate::models::GridBot) -> GridBotConfig {
    GridBotConfig {
        id: bot.id,
        user_id: bot.user_id,
        name: bot.name.clone(),
        symbol: bot.symbol.clone(),
        exchange: bot.exchange.clone(),
        grid_count: bot.grid_count,
        upper_price: bot.upper_price,
        lower_price: bot.lower_price,
        grid_profit_pct: bot.grid_profit_pct,
        quantity_per_grid: bot.quantity_per_grid,
        leverage: bot.leverage,
        dynamic_adjust: bot.dynamic_adjust,
        adjust_interval_secs: bot.adjust_interval_secs,
        market_regime: bot.market_regime.clone(),
        grid_levels_json: bot.grid_levels_json.clone(),
        system_prompt: bot.system_prompt.clone(),
        last_adjusted_at: bot.last_adjusted_at,
    }
}
