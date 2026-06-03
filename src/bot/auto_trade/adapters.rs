use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::bot::auto_trade::ports::*;
use crate::bot::auto_trade::types::{AutoBot, AutoBotConfig, MarketType};
use crate::config::AiConfig;
use crate::engine::kline::KlineEngine;
use crate::engine::kline::types::Timeframe;
use crate::models::Kline;
use crate::trading::exchange::registry::ExchangeRegistry;

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
    async fn get_price(&self, exchange: &str, symbol: &str, _market_type: &str) -> Option<f64> {
        self.inner.get_price(exchange, symbol).await
    }
}

pub struct ExchangeMarketDataProvider {
    exchange_registry: Arc<ExchangeRegistry>,
    kline_engine: Option<Arc<KlineEngine>>,
    db: Option<PgPool>,
    encryption_key: Option<String>,
    paper_executor: Option<Arc<crate::trading::paper::PaperOrderExecutor>>,
    paper_balance: Arc<tokio::sync::RwLock<Option<AccountBalance>>>,
}

impl ExchangeMarketDataProvider {
    pub fn new(exchange_registry: Arc<ExchangeRegistry>) -> Self {
        Self { exchange_registry, kline_engine: None, db: None, encryption_key: None, paper_executor: None, paper_balance: Arc::new(tokio::sync::RwLock::new(None)) }
    }

    pub fn with_kline_engine(mut self, engine: Arc<KlineEngine>) -> Self {
        self.kline_engine = Some(engine);
        self
    }

    pub fn with_db(mut self, db: PgPool, encryption_key: String) -> Self {
        self.db = Some(db);
        self.encryption_key = Some(encryption_key);
        self
    }

    pub fn with_paper_executor(mut self, paper: Arc<crate::trading::paper::PaperOrderExecutor>) -> Self {
        self.paper_executor = Some(paper);
        self
    }

    async fn ensure_exchange(&self, exchange: &str, market_type: &str) {
        let exchange_key = format!("{}:{}", exchange, market_type);
        if self.exchange_registry.get(&exchange_key).is_some() {
            return;
        }

        let db = match self.db {
            Some(ref db) => db,
            None => return,
        };
        let ek = match self.encryption_key {
            Some(ref ek) => ek,
            None => return,
        };

        let row: Option<(String, String, Option<String>)> = sqlx::query_as(
            r#"SELECT encrypted_api_key, encrypted_api_secret, encrypted_passphrase
               FROM qd_exchange_credentials
               WHERE exchange = $1 AND market_type = $2 LIMIT 1"#,
        )
        .bind(exchange)
        .bind(market_type)
        .fetch_optional(db)
        .await
        .unwrap_or(None);

        if let Some((enc_key, enc_secret, enc_passphrase)) = row {
            let derived_key = crate::utils::crypto::derive_key(ek);
            let api_key = match crate::utils::crypto::decrypt(&enc_key, &derived_key) {
                Ok(k) => k,
                Err(_) => return,
            };
            let api_secret = match crate::utils::crypto::decrypt(&enc_secret, &derived_key) {
                Ok(s) => s,
                Err(_) => return,
            };
            let passphrase = enc_passphrase.and_then(|p| crate::utils::crypto::decrypt(&p, &derived_key).ok());

            let mt = match market_type {
                "spot" => crate::models::MarketType::Spot,
                _ => crate::models::MarketType::Perpetual,
            };

            if let Ok(ex) = crate::trading::exchange::ExchangeFactory::create(
                exchange, &api_key, &api_secret, passphrase.as_deref(), None, mt,
            ) {
                self.exchange_registry.register(ex);
                tracing::info!(exchange, market_type, "Auto-registered exchange for market data provider");
            }
        }
    }

    async fn fetch_klines(
        &self,
        exchange: &str,
        symbol: &str,
        timeframe: Timeframe,
        min_count: usize,
        interval_str: &str,
        start_ms: i64,
        market_type: &str,
        required: bool,
    ) -> Option<Vec<Kline>> {
        if let Some(klines) = self.get_klines_from_cache(exchange, symbol, timeframe, min_count).await {
            tracing::info!(exchange, symbol, interval = interval_str, count = klines.len(), source = "cache", "Fetched klines");
            return Some(klines);
        }

        let exchange_key = format!("{}:{}", exchange, market_type);
        let ex = self.exchange_registry.get(&exchange_key);
        let result = match ex {
            Some(ref ex) => match ex.get_klines_range(symbol, interval_str, start_ms, chrono::Utc::now().timestamp_millis()).await {
                Ok(k) if k.len() >= min_count => Some(k),
                Ok(k) if !required => Some(k),
                Ok(k) => {
                    warn!(exchange, symbol, count = k.len(), required = min_count, "{} klines insufficient", interval_str);
                    if required { None } else { Some(k) }
                }
                Err(e) => {
                    warn!(exchange, symbol, error = %e, "Failed to fetch {} klines", interval_str);
                    if required { None } else { Some(vec![]) }
                }
            },
            None => {
                warn!(exchange, symbol, "No exchange for {} klines", interval_str);
                if required { None } else { Some(vec![]) }
            }
        };
        if let Some(ref klines) = result {
            tracing::info!(exchange, symbol, interval = interval_str, count = klines.len(), source = "rest", "Fetched klines");
        }
        result
    }

    async fn get_klines_from_cache(
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
                    "KlineEngine cache insufficient"
                );
            }
        }
        None
    }

    async fn fetch_current_price(&self, exchange: &str, symbol: &str, market_type: &str, klines_1h: &[Kline]) -> f64 {
        if let Some(ref engine) = self.kline_engine {
            if let Some(candles) = engine.get_klines_async(exchange, symbol, crate::engine::kline::types::Timeframe::M1).await {
                if let Some(last) = candles.last() {
                    if last.close > 0.0 {
                        return last.close;
                    }
                }
            }
        }

        let exchange_key = format!("{}:{}", exchange, market_type);
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
    async fn get_market_snapshot(&self, exchange: &str, symbol: &str, market_type: &str) -> MarketSnapshot {
        self.ensure_exchange(exchange, market_type).await;

        let now_ms = chrono::Utc::now().timestamp_millis();

        let klines_1h = match self.fetch_klines(
            exchange, symbol, Timeframe::H1, 30, "1h",
            now_ms - 200 * 3600 * 1000, market_type, true,
        ).await {
            Some(k) => k,
            None => return MarketSnapshot::default(),
        };

        let klines_4h = self.fetch_klines(
            exchange, symbol, Timeframe::H4, 50, "4h",
            now_ms - 100 * 4 * 3600 * 1000, market_type, false,
        ).await.unwrap_or_default();

        let klines_15m = self.fetch_klines(
            exchange, symbol, Timeframe::M15, 50, "15m",
            now_ms - 200 * 15 * 60 * 1000, market_type, false,
        ).await.unwrap_or_default();

        let current_price = self.fetch_current_price(exchange, symbol, market_type, &klines_1h).await;

        let exchange_key = format!("{}:{}", exchange, market_type);
        let (funding_rate, funding_next_time) = if market_type == "perpetual" {
            if let Some(ex) = self.exchange_registry.get(&exchange_key) {
                match ex.get_funding_rate(symbol).await {
                    Ok(fr) => {
                        let next = fr.next_funding_time
                            .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                            .unwrap_or_else(|| "N/A".to_string());
                        (fr.rate, next)
                    }
                    Err(_) => (0.0, "N/A".to_string()),
                }
            } else {
                (0.0, "N/A".to_string())
            }
        } else {
            (0.0, "N/A".to_string())
        };

        let ind = crate::bot::common::indicators::compute_market_indicators(
            &klines_1h,
            &klines_4h,
            &klines_15m,
            funding_rate,
            "N/A".to_string(),
        );

        let effective_price = if current_price > 0.0 { current_price } else { ind.current_price };

        // 获取最小交易数量
        let min_qty = if let Some(ex) = self.exchange_registry.get(&exchange_key) {
            match ex.get_min_qty(symbol).await {
                Ok(qty) => qty,
                Err(_) => 0.0,
            }
        } else {
            0.0
        };

        // 获取强平价格（仅合约且有持仓时）
        let liquidation_price = if market_type == "perpetual" {
            if let Some(ex) = self.exchange_registry.get(&exchange_key) {
                match ex.get_positions(Some(symbol)).await {
                    Ok(positions) => positions.iter()
                        .find(|p| p.symbol.as_str() == symbol && p.size.abs() > 0.0)
                        .and_then(|p| p.liquidation_price),
                    Err(_) => None,
                }
            } else {
                None
            }
        } else {
            None
        };

        MarketSnapshot {
            current_price: effective_price,
            funding_rate,
            funding_next_time,
            indicators: ind,
            min_qty,
            liquidation_price,
        }
    }

    async fn get_account_balance(&self, exchange: &str, market_type: &str) -> AccountBalance {
        if let Some(ref paper) = self.paper_executor {
            if paper.is_enabled() {
                {
                    let cached = self.paper_balance.read().await;
                    if let Some(ref balance) = *cached {
                        return balance.clone();
                    }
                }

                self.ensure_exchange(exchange, market_type).await;
                let exchange_key = format!("{}:{}", exchange, market_type);
                let real_balance = if let Some(ex) = self.exchange_registry.get(&exchange_key) {
                    match ex.get_balances().await {
                        Ok(bs) => {
                            let usdt = bs.iter().find(|b| b.asset.eq_ignore_ascii_case("USDT"));
                            usdt.map(|b| AccountBalance { total: b.total, free: b.free, used: b.used })
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "Paper mode: failed to fetch real balance, balance unavailable");
                            None
                        }
                    }
                } else {
                    tracing::warn!("Paper mode: exchange not available, balance unavailable");
                    None
                };

                let balance = real_balance.unwrap_or(AccountBalance::default());
                {
                    let mut cached = self.paper_balance.write().await;
                    *cached = Some(balance.clone());
                }
                tracing::info!(total = balance.total, free = balance.free, "Paper mode: initialized balance from exchange");
                return balance;
            }
        }

        self.ensure_exchange(exchange, market_type).await;

        let exchange_key = format!("{}:{}", exchange, market_type);
        let ex = match self.exchange_registry.get(&exchange_key) {
            Some(e) => e,
            None => return AccountBalance::default(),
        };

        match ex.get_balances().await {
            Ok(bs) => {
                let usdt = bs.iter().find(|b| b.asset.eq_ignore_ascii_case("USDT"));
                match usdt {
                    Some(b) => AccountBalance { total: b.total, free: b.free, used: b.used },
                    None => AccountBalance::default(),
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "get_account_balance error");
                AccountBalance::default()
            }
        }
    }
}

pub struct PgAutoStore {
    db: PgPool,
}

impl PgAutoStore {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl AutoStore for PgAutoStore {
    async fn load_running_bots(&self) -> anyhow::Result<Vec<AutoBotConfig>> {
        let bots: Vec<AutoBot> =
            sqlx::query_as("SELECT * FROM qd_auto_bots WHERE status = 'running'")
                .fetch_all(&self.db)
                .await?;
        Ok(bots.iter().map(bot_to_config).collect())
    }

    async fn load_bot(&self, bot_id: Uuid) -> anyhow::Result<Option<AutoBotConfig>> {
        let bot: Option<AutoBot> =
            sqlx::query_as("SELECT * FROM qd_auto_bots WHERE id = $1")
                .bind(bot_id)
                .fetch_optional(&self.db)
                .await?;
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
            .bind(bot_id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    async fn update_position(
        &self,
        bot_id: Uuid,
        current_side: Option<&str>,
        entry_price: f64,
        position_size: f64,
        stop_loss: f64,
        take_profit: f64,
        liquidation_price: Option<f64>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"UPDATE qd_auto_bots SET
                current_side = $2, entry_price = $3, position_size = $4,
                stop_loss = $5, take_profit = $6,
                liquidation_price = $7, updated_at = NOW()
               WHERE id = $1"#,
        )
        .bind(bot_id)
        .bind(current_side)
        .bind(entry_price)
        .bind(position_size)
        .bind(stop_loss)
        .bind(take_profit)
        .bind(liquidation_price)
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
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"UPDATE qd_auto_bots SET
                market_regime = $2, leverage = $3, ai_analysis = $4,
                updated_at = NOW()
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
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"UPDATE qd_auto_bots SET
                total_pnl = $2, total_trades = $3, win_trades = $4, loss_trades = $5,
                updated_at = NOW()
               WHERE id = $1"#,
        )
        .bind(bot_id)
        .bind(total_pnl)
        .bind(total_trades)
        .bind(win_trades)
        .bind(loss_trades)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn record_trade(
        &self,
        bot_id: Uuid,
        user_id: Uuid,
        symbol: &str,
        exchange: &str,
        side: &str,
        trade_type: &str,
        trigger_source: &str,
        price: f64,
        quantity: f64,
        pnl: f64,
        pnl_pct: f64,
        exchange_order_id: Option<&str>,
    ) -> anyhow::Result<Uuid> {
        let pnl_pct = if pnl_pct.is_nan() { 0.0 } else { pnl_pct };
        let row: (Uuid,) = sqlx::query_as(
            r#"INSERT INTO qd_auto_trades (bot_id, user_id, symbol, exchange, side, trade_type, trigger_source, price, quantity, pnl, pnl_pct, exchange_order_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
               RETURNING id"#,
        )
        .bind(bot_id)
        .bind(user_id)
        .bind(symbol)
        .bind(exchange)
        .bind(side)
        .bind(trade_type)
        .bind(trigger_source)
        .bind(price)
        .bind(quantity)
        .bind(pnl)
        .bind(pnl_pct)
        .bind(exchange_order_id)
        .fetch_one(&self.db)
        .await?;
        Ok(row.0)
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
            r#"INSERT INTO qd_auto_analysis_logs (bot_id, analysis_type, system_prompt, user_prompt, status, result, error, completed_at)
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

    async fn load_analysis_logs(&self, bot_id: Uuid) -> anyhow::Result<Vec<AutoAnalysisLogEntry>> {
        let rows: Vec<(Uuid, Uuid, String, String, String, serde_json::Value, Option<String>, chrono::DateTime<chrono::Utc>)> =
            sqlx::query_as(
                r#"SELECT id, bot_id, analysis_type, system_prompt, user_prompt, result, error, created_at
                   FROM qd_auto_analysis_logs WHERE bot_id = $1 ORDER BY created_at DESC LIMIT 50"#,
            )
            .bind(bot_id)
            .fetch_all(&self.db)
            .await?;

        Ok(rows.into_iter().map(|r| AutoAnalysisLogEntry {
            id: r.0, bot_id: r.1, analysis_type: r.2,
            system_prompt: r.3, user_prompt: r.4, result: r.5,
            error: r.6, created_at: r.7,
        }).collect())
    }

    async fn load_consecutive_losses(&self, bot_id: Uuid) -> anyhow::Result<i32> {
        let pnl_rows: Vec<(f64,)> = sqlx::query_as(
            r#"SELECT pnl FROM qd_auto_trades
               WHERE bot_id = $1 AND pnl != 0
               ORDER BY created_at DESC LIMIT 20"#
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

    async fn delete_bot(&self, bot_id: Uuid) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM qd_auto_bots WHERE id = $1")
            .bind(bot_id)
            .execute(&self.db)
            .await?;
        Ok(())
    }
}

fn bot_to_config(bot: &AutoBot) -> AutoBotConfig {
    AutoBotConfig {
        id: bot.id,
        user_id: bot.user_id,
        name: bot.name.clone(),
        symbol: bot.symbol.clone(),
        exchange: bot.exchange.clone(),
        market_type: MarketType::from_str_lossy(&bot.market_type),
        leverage: bot.leverage,
        max_position_pct: bot.max_position_pct,
        decide_interval_secs: bot.decide_interval_secs,
        current_side: match bot.current_side.as_deref() {
            Some("long") | Some("short") => bot.current_side.clone(),
            _ => Some("none".to_string()),
        },
        entry_price: bot.entry_price,
        position_size: bot.position_size,
        stop_loss: bot.stop_loss,
        take_profit: bot.take_profit,
        unrealized_pnl: bot.unrealized_pnl,
        liquidation_price: bot.liquidation_price,
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
