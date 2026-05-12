//! Grid Engine 的 Adapter 实现
//!
//! 将外部模块（ExchangeRegistry, Position Engine, Database, AiService 等）
//! 适配为 semi_automatic_grid 模块定义的 trait 接口。

use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::bot::semi_automatic_grid::ports::*;
use crate::config::AiConfig;
use crate::trading::exchange::registry::ExchangeRegistry;
use crate::engine::position::types as pe_types;
use crate::services::ai::{AiService, AiUserConfig};
use crate::indicators;
use crate::models::Kline;

pub struct ExchangePriceProvider {
    exchange_registry: Arc<ExchangeRegistry>,
}

impl ExchangePriceProvider {
    pub fn new(exchange_registry: Arc<ExchangeRegistry>) -> Self {
        Self { exchange_registry }
    }
}

#[async_trait]
impl PriceProvider for ExchangePriceProvider {
    async fn get_price(&self, exchange: &str, symbol: &str) -> Option<f64> {
        let exchange_key = format!("{}:perpetual", exchange);
        let ex = self.exchange_registry.get(&exchange_key)?;
        match ex.get_ticker(symbol).await {
            Ok(ticker) if ticker.last > 0.0 => Some(ticker.last),
            _ => None,
        }
    }
}

// ── MarketDataProvider ──

pub struct ExchangeMarketDataProvider {
    exchange_registry: Arc<ExchangeRegistry>,
}

impl ExchangeMarketDataProvider {
    pub fn new(exchange_registry: Arc<ExchangeRegistry>) -> Self {
        Self { exchange_registry }
    }
}

#[async_trait]
impl MarketDataProvider for ExchangeMarketDataProvider {
    async fn get_market_snapshot(&self, exchange: &str, symbol: &str) -> MarketSnapshot {
        let exchange_key = format!("{}:perpetual", exchange);
        let ex = match self.exchange_registry.get(&exchange_key) {
            Some(e) => e,
            None => {
                warn!(exchange, symbol, exchange_key, "Exchange not found in registry, returning empty snapshot");
                return MarketSnapshot::default();
            }
        };

        let now_ms = chrono::Utc::now().timestamp_millis();
        let start_1h = now_ms - 200 * 3600 * 1000;
        let start_4h = now_ms - 50 * 4 * 3600 * 1000;
        let start_15m = now_ms - 200 * 15 * 60 * 1000;

        let klines_1h = match ex.get_klines_range(symbol, "1h", start_1h, now_ms).await {
            Ok(k) if k.len() >= 30 => k,
            Ok(k) => {
                warn!(exchange, symbol, count = k.len(), "1h klines insufficient (< 30), returning empty snapshot");
                return MarketSnapshot::default();
            }
            Err(e) => {
                warn!(exchange, symbol, error = %e, "Failed to fetch 1h klines, returning empty snapshot");
                return MarketSnapshot::default();
            }
        };

        let klines_4h = match ex.get_klines_range(symbol, "4h", start_4h, now_ms).await {
            Ok(k) => k,
            Err(_) => vec![],
        };

        let klines_15m = match ex.get_klines_range(symbol, "15m", start_15m, now_ms).await {
            Ok(k) => k,
            Err(_) => vec![],
        };

        let last_idx = klines_1h.len().saturating_sub(1);
        let current_price = klines_1h.last().map(|k| k.close).unwrap_or(0.0);

        let rsi = indicators::rsi_at(&klines_1h, last_idx, 14);
        let atr = indicators::atr_at(&klines_1h, last_idx, 14);
        let atr_pct = if current_price > 0.0 { atr / current_price * 100.0 } else { 0.0 };
        let bb_width = indicators::bbands_width_at(&klines_1h, last_idx, 20, 2.0);
        let (bb_upper, bb_middle, bb_lower) = indicators::bbands_at(&klines_1h, last_idx, 20, 2.0);

        let ema12 = indicators::ema_at(&klines_1h, last_idx, 12);
        let ema20 = indicators::ema_at(&klines_1h, last_idx, 20);
        let ema26 = indicators::ema_at(&klines_1h, last_idx, 26);
        let ema50 = if klines_1h.len() >= 50 { indicators::ema_at(&klines_1h, last_idx, 50) } else { 0.0 };

        let lookback = 5.min(last_idx);
        let ema12_prev = indicators::ema_at(&klines_1h, last_idx.saturating_sub(lookback), 12);
        let ema26_prev = indicators::ema_at(&klines_1h, last_idx.saturating_sub(lookback), 26);
        let ema12_trend = if ema12 > ema12_prev { "上升" } else if ema12 < ema12_prev { "下降" } else { "横盘" };
        let ema26_trend = if ema26 > ema26_prev { "上升" } else if ema26 < ema26_prev { "下降" } else { "横盘" };

        let price_high: f64 = klines_1h.iter().map(|k| k.high).fold(f64::NEG_INFINITY, f64::max);
        let price_low: f64 = klines_1h.iter().map(|k| k.low).fold(f64::INFINITY, f64::min);

        let change_1h = if last_idx >= 1 && klines_1h[last_idx.saturating_sub(1)].close > 0.0 {
            (current_price - klines_1h[last_idx.saturating_sub(1)].close) / klines_1h[last_idx.saturating_sub(1)].close * 100.0
        } else { 0.0 };

        let change_4h = if last_idx >= 4 && klines_1h[last_idx.saturating_sub(4)].close > 0.0 {
            (current_price - klines_1h[last_idx.saturating_sub(4)].close) / klines_1h[last_idx.saturating_sub(4)].close * 100.0
        } else { 0.0 };

        let last_24: &[Kline] = if klines_1h.len() >= 24 {
            &klines_1h[klines_1h.len() - 24..]
        } else {
            &klines_1h
        };
        let high_24: f64 = last_24.iter().map(|k| k.high).fold(f64::NEG_INFINITY, f64::max);
        let low_24: f64 = last_24.iter().map(|k| k.low).fold(f64::INFINITY, f64::min);
        let volatility = if low_24 > 0.0 { (high_24 - low_24) / low_24 * 100.0 } else { 0.0 };
        let change_24h = if last_24.first().map(|k| k.close).unwrap_or(0.0) > 0.0 {
            (current_price - last_24.first().unwrap().close) / last_24.first().unwrap().close * 100.0
        } else { 0.0 };

        let macd = indicators::macd_at(&klines_1h, last_idx, 12, 26);
        let macd_signal = indicators::macd_signal_at(&klines_1h, last_idx, 12, 26, 9);
        let adx = indicators::adx_at(&klines_1h, last_idx, 14);

        let funding_rate = ex.get_funding_rate(symbol).await.map(|fr| fr.rate).unwrap_or(0.0);

        let h1_atr_sma20 = if klines_1h.len() >= 20 {
            let atr_series = indicators::atr(&klines_1h, 14);
            indicators::sma_at_from(&atr_series, last_idx, 20)
        } else { 0.0 };

        let h1_candle_body = klines_1h.last().map(|k| k.close - k.open).unwrap_or(0.0);

        let h1_bars_outside_band = indicators::compute_bars_outside_band(&klines_1h, bb_upper, bb_lower);

        let h1_bandwidth_5bars_ago = if last_idx >= 5 {
            indicators::bbands_width_at(&klines_1h, last_idx.saturating_sub(5), 20, 2.0)
        } else { 0.0 };

        let h1_high_20 = indicators::highest_at(&klines_1h, last_idx, 20);
        let h1_low_20 = indicators::lowest_at(&klines_1h, last_idx, 20);

        let nearest_round_up = indicators::find_round_number(current_price, true);
        let nearest_round_down = indicators::find_round_number(current_price, false);

        let h4_last = klines_4h.len().saturating_sub(1);
        let h4_ema20 = if !klines_4h.is_empty() { indicators::ema_at(&klines_4h, h4_last, 20) } else { 0.0 };
        let h4_ema50 = if klines_4h.len() >= 50 { indicators::ema_at(&klines_4h, h4_last, 50) } else { 0.0 };
        let h4_adx = if !klines_4h.is_empty() { indicators::adx_at(&klines_4h, h4_last, 14) } else { 0.0 };
        let h4_bb_width_pct = if !klines_4h.is_empty() { indicators::bbands_width_at(&klines_4h, h4_last, 20, 2.0) } else { 0.0 };

        let m15_last = klines_15m.len().saturating_sub(1);
        let m15_current_price = klines_15m.last().map(|k| k.close).unwrap_or(0.0);
        let m15_bb_width_pct = if !klines_15m.is_empty() { indicators::bbands_width_at(&klines_15m, m15_last, 20, 2.0) } else { 0.0 };
        let m15_atr = if !klines_15m.is_empty() { indicators::atr_at(&klines_15m, m15_last, 14) } else { 0.0 };
        let m15_atr_sma20 = if klines_15m.len() >= 20 {
            let atr_series = indicators::atr(&klines_15m, 14);
            indicators::sma_at_from(&atr_series, m15_last, 20)
        } else { 0.0 };
        let m15_adx = if !klines_15m.is_empty() { indicators::adx_at(&klines_15m, m15_last, 14) } else { 0.0 };
        let (m15_bb_upper, _, m15_bb_lower) = if !klines_15m.is_empty() {
            indicators::bbands_at(&klines_15m, m15_last, 20, 2.0)
        } else { (0.0, 0.0, 0.0) };
        let m15_bars_outside_band = indicators::compute_bars_outside_band(&klines_15m, m15_bb_upper, m15_bb_lower);
        let m15_ema20 = if !klines_15m.is_empty() { indicators::ema_at(&klines_15m, m15_last, 20) } else { 0.0 };
        let m15_ema50 = if klines_15m.len() >= 50 { indicators::ema_at(&klines_15m, m15_last, 50) } else { 0.0 };

        MarketSnapshot {
            current_price,
            rsi,
            atr,
            atr_pct,
            bb_width,
            bb_upper,
            bb_middle,
            bb_lower,
            ema12,
            ema12_trend: ema12_trend.to_string(),
            ema20,
            ema26,
            ema26_trend: ema26_trend.to_string(),
            ema50,
            ema_4h: h4_ema20,
            volatility,
            change_1h,
            change_4h,
            change_24h,
            funding_rate,
            macd,
            macd_signal,
            adx,
            price_high,
            price_low,
            h1_atr_sma20,
            h1_candle_body,
            h1_bars_outside_band,
            h1_bandwidth_5bars_ago,
            h1_high_20,
            h1_low_20,
            nearest_round_up,
            nearest_round_down,
            m15_current_price,
            m15_bb_width_pct,
            m15_atr,
            m15_atr_sma20,
            m15_adx,
            m15_bars_outside_band,
            m15_ema20,
            m15_ema50,
            h4_ema20,
            h4_ema50,
            h4_adx,
            h4_bb_width_pct,
        }
    }

    async fn get_account_balance(&self, exchange: &str) -> super::ports::AccountBalance {
        use super::ports::AccountBalance;
        
        let exchange_key = format!("{}:perpetual", exchange);
        tracing::info!("[get_account_balance] Looking for exchange_key: {}", exchange_key);
        
        let ex = match self.exchange_registry.get(&exchange_key) {
            Some(e) => {
                tracing::info!("[get_account_balance] Found exchange in registry");
                e
            },
            None => {
                tracing::warn!("[get_account_balance] Exchange NOT found in registry, returning default");
                return AccountBalance::default();
            },
        };
        
        tracing::info!("[get_account_balance] Calling ex.get_balances()...");
        match ex.get_balances().await {
            Ok(bs) => {
                tracing::info!("[get_account_balance] get_balances returned {} balances", bs.len());
                let usdt = bs.iter().find(|b| b.asset.eq_ignore_ascii_case("USDT"));
                match usdt {
                    Some(b) => {
                        tracing::info!("[get_account_balance] USDT balance: total={}, free={}, used={}", b.total, b.free, b.used);
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

// ── OrderExecutor ──

pub struct PeOrderExecutor {
    pe_cmd_tx: tokio::sync::mpsc::Sender<pe_types::EngineCommand>,
    exchange_registry: Arc<ExchangeRegistry>,
}

impl PeOrderExecutor {
    pub fn new(pe_cmd_tx: tokio::sync::mpsc::Sender<pe_types::EngineCommand>, exchange_registry: Arc<ExchangeRegistry>) -> Self {
        Self { pe_cmd_tx, exchange_registry }
    }
}

#[async_trait]
impl OrderExecutor for PeOrderExecutor {
    async fn send_command(&self, command: OrderCommand) -> anyhow::Result<()> {
        let pe_cmd = match command {
            OrderCommand::PlaceOrder { symbol, side, amount, price, reduce_only, client_order_id } => {
                pe_types::EngineCommand::PlaceOrder {
                    params: pe_types::PlaceOrderParams {
                        symbol,
                        side: match side {
                            OrderSide::Buy => pe_types::Side::Buy,
                            OrderSide::Sell => pe_types::Side::Sell,
                        },
                        order_type: pe_types::OrderType::Limit,
                        amount,
                        price,
                        reduce_only,
                        position_side: Some(pe_types::PositionSide::Long),
                        position_id: None,
                        client_order_id,
                    },
                }
            }
            OrderCommand::CancelAllOrders { symbol } => {
                pe_types::EngineCommand::CancelAllOrders {
                    position_id: None,
                    symbol,
                }
            }
            OrderCommand::CloseAllPositions { symbol } => {
                let exchange_key = format!("perpetual:{}", symbol);
                if let Some(ex) = self.exchange_registry.get(&exchange_key) {
                    match ex.get_positions(Some(&symbol)).await {
                        Ok(positions) => {
                            for pos in positions {
                                if pos.symbol == symbol && pos.size.abs() > 0.0 {
                                    let is_long = pos.size > 0.0;
                                    let close_cmd = pe_types::EngineCommand::PlaceOrder {
                                        params: pe_types::PlaceOrderParams {
                                            symbol: symbol.clone(),
                                            side: if is_long { pe_types::Side::Sell } else { pe_types::Side::Buy },
                                            order_type: pe_types::OrderType::Market,
                                            amount: pos.size.abs(),
                                            price: None,
                                            reduce_only: true,
                                            position_side: Some(if is_long { pe_types::PositionSide::Long } else { pe_types::PositionSide::Short }),
                                            position_id: None,
                                            client_order_id: Some(format!("grid:close:{}", symbol)),
                                        },
                                    };
                                    if let Err(e) = self.pe_cmd_tx.send(close_cmd).await {
                                        warn!(symbol = %symbol, error = %e, "Failed to send close position order");
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!(symbol = %symbol, error = %e, "Failed to get positions for CloseAllPositions");
                        }
                    }
                } else {
                    warn!(exchange_key = %exchange_key, "Exchange not found for CloseAllPositions");
                }
                return Ok(());
            }
        };
        self.pe_cmd_tx.send(pe_cmd).await?;
        Ok(())
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
            "SELECT * FROM qd_grid_trades WHERE bot_id = $1 AND status = 'filled' ORDER BY created_at",
        )
        .bind(bot_id)
        .fetch_all(&self.db)
        .await?;

        Ok(trades
            .into_iter()
            .map(|t| GridTradeRecord {
                grid_level: t.grid_level,
                side: t.side,
                price: t.price,
                quantity: t.quantity,
                pnl: t.pnl,
            })
            .collect())
    }

    async fn record_trade(
        &self,
        bot_id: Uuid,
        user_id: Uuid,
        symbol: &str,
        exchange: &str,
        side: &str,
        grid_level: i32,
        price: f64,
        quantity: f64,
        pnl: f64,
        pnl_pct: f64,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"INSERT INTO qd_grid_trades (bot_id, user_id, symbol, exchange, side, grid_level, price, quantity, pnl, pnl_pct, status)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'filled')"#,
        )
        .bind(bot_id)
        .bind(user_id)
        .bind(symbol)
        .bind(exchange)
        .bind(side)
        .bind(grid_level)
        .bind(price)
        .bind(quantity)
        .bind(pnl)
        .bind(pnl_pct)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn save_stats(&self, bot_id: Uuid, total_pnl: f64, total_trades: i32, grid_filled_count: i32) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE qd_grid_bots SET total_pnl = $2, total_trades = $3, grid_filled_count = $4, updated_at = NOW() WHERE id = $1",
        )
        .bind(bot_id)
        .bind(total_pnl)
        .bind(total_trades)
        .bind(grid_filled_count)
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
        grid_levels_json: Option<&str>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"UPDATE qd_grid_bots SET
                market_regime = $1, upper_price = $2, lower_price = $3,
                grid_count = $4, grid_profit_pct = $5, quantity_per_grid = $6,
                leverage = $7, ai_analysis = $8, grid_levels_json = $9,
                last_adjusted_at = NOW(), updated_at = NOW()
               WHERE id = $10"#,
        )
        .bind(market_regime)
        .bind(upper_price)
        .bind(lower_price)
        .bind(grid_count)
        .bind(grid_profit_pct)
        .bind(quantity_per_grid)
        .bind(leverage)
        .bind(ai_analysis)
        .bind(grid_levels_json)
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
    }
}

// ── CredentialStore ──

pub struct PgCredentialStore {
    db: PgPool,
    encryption_key: [u8; 32],
}

impl PgCredentialStore {
    pub fn new(db: PgPool, encryption_key: [u8; 32]) -> Self {
        Self { db, encryption_key }
    }
}

#[async_trait]
impl CredentialStore for PgCredentialStore {
    async fn load_credentials(&self, user_id: Uuid) -> anyhow::Result<Vec<(String, String)>> {
        #[derive(Debug, sqlx::FromRow)]
        struct Row {
            provider: String,
            encrypted_api_key: String,
        }

        let rows = sqlx::query_as::<_, Row>(
            r#"SELECT provider, encrypted_api_key FROM qd_ai_credentials WHERE user_id = $1"#,
        )
        .bind(user_id)
        .fetch_all(&self.db)
        .await?;

        let mut result = Vec::new();
        for row in rows {
            let decrypted = crate::utils::crypto::decrypt(&row.encrypted_api_key, &self.encryption_key)?;
            result.push((row.provider, decrypted));
        }
        Ok(result)
    }
}

// ── LlmProviderResolver ──

pub struct DefaultLlmResolver {
    ai_service: AiService,
    ai_config: AiConfig,
}

impl DefaultLlmResolver {
    pub fn new(ai_config: AiConfig) -> Self {
        let ai_service = AiService::new(ai_config.clone());
        Self { ai_service, ai_config }
    }
}

impl LlmProviderResolver for DefaultLlmResolver {
    fn is_available(&self) -> bool {
        let config = AiUserConfig::default();
        self.ai_service.is_configured_with_override(&config)
    }

    fn resolve(
        &self,
        user_credentials: &[(String, String)],
    ) -> anyhow::Result<(String, String, String, String)> {
        // 构建用户凭证配置
        let mut user_config = AiUserConfig::default();
        for (provider, key) in user_credentials {
            match provider.as_str() {
                "openrouter" => user_config.openrouter_api_key = Some(key.clone()),
                "openai" => user_config.openai_api_key = Some(key.clone()),
                "deepseek" => user_config.deepseek_api_key = Some(key.clone()),
                _ => {}
            }
        }

        let default_config = AiUserConfig::default();
        let effective_config = if self.ai_service.is_configured_with_override(&user_config) {
            &user_config
        } else {
            &default_config
        };

        let provider = self.ai_service.default_provider_with_override(effective_config);
        let (api_key, base_url, model) = self
            .ai_service
            .resolve_provider_with_override(&provider, None, effective_config)?;

        Ok((api_key, base_url, model, provider.to_string()))
    }
}

// ── PE 事件转换 ──

/// 将 Position Engine 的 EngineEvent 转换为 OrderEvent
pub fn convert_pe_event(event: pe_types::EngineEvent) -> Option<OrderEvent> {
    match event {
        pe_types::EngineEvent::OrderPlaced { order } => Some(OrderEvent::OrderPlaced {
            order: OrderInfo {
                id: order.id,
                symbol: order.symbol.clone(),
                side: match order.side {
                    pe_types::Side::Buy => OrderSide::Buy,
                    pe_types::Side::Sell => OrderSide::Sell,
                },
                fill_price: order.fill_price,
                request_price: order.request_price,
                filled: order.filled,
                client_order_id: order.client_order_id.clone(),
            },
        }),
        pe_types::EngineEvent::OrderFilled { order, trade: _ } => Some(OrderEvent::OrderFilled {
            order: OrderInfo {
                id: order.id,
                symbol: order.symbol.clone(),
                side: match order.side {
                    pe_types::Side::Buy => OrderSide::Buy,
                    pe_types::Side::Sell => OrderSide::Sell,
                },
                fill_price: order.fill_price,
                request_price: order.request_price,
                filled: order.filled,
                client_order_id: order.client_order_id.clone(),
            },
        }),
        pe_types::EngineEvent::OrderCanceled { order } => Some(OrderEvent::OrderCanceled {
            order_id: order.id,
            symbol: Some(order.symbol.clone()),
        }),
        pe_types::EngineEvent::OrderFailed { order_id, reason } => Some(OrderEvent::OrderFailed {
            order_id,
            reason,
        }),
        pe_types::EngineEvent::RiskAlert { level, message } => Some(OrderEvent::RiskAlert { level, message }),
        pe_types::EngineEvent::LiquidationWarning { symbol, liquidation_price, current_price, .. } => {
            Some(OrderEvent::LiquidationWarning { symbol, liquidation_price, current_price })
        }
        _ => None,
    }
}

// ── SwitchableOrderExecutor ──

/// 可切换的订单执行器
///
/// 根据 paper 模式开关，将命令转发到真实执行器或 Paper 执行器。
pub struct SwitchableOrderExecutor {
    real: Arc<dyn OrderExecutor>,
    paper: Arc<crate::trading::paper::PaperOrderExecutor>,
}

impl SwitchableOrderExecutor {
    pub fn new(
        real: Arc<dyn OrderExecutor>,
        paper: Arc<crate::trading::paper::PaperOrderExecutor>,
    ) -> Self {
        Self { real, paper }
    }
}

#[async_trait]
impl OrderExecutor for SwitchableOrderExecutor {
    async fn send_command(&self, command: OrderCommand) -> anyhow::Result<()> {
        if self.paper.is_enabled() {
            self.paper.send_command(command).await
        } else {
            self.real.send_command(command).await
        }
    }
}

