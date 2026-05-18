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
use crate::engine::position::types as pe_types;
use crate::engine::kline::KlineEngine;
use crate::engine::kline::types::{Candle, Timeframe};
use crate::services::ai::{AiService, AiUserConfig};
use crate::models::Kline;

fn candle_to_kline(c: &Candle) -> Kline {
    Kline {
        open_time: c.open_time,
        open: c.open,
        high: c.high,
        low: c.low,
        close: c.close,
        volume: c.volume,
        close_time: c.close_time,
        quote_volume: c.quote_volume,
        trades: c.trades,
        symbol: String::new(),
        exchange: String::new(),
        interval: String::new(),
    }
}

pub struct ExchangePriceProvider {
    exchange_registry: Arc<ExchangeRegistry>,
    kline_engine: Option<Arc<KlineEngine>>,
}

impl ExchangePriceProvider {
    pub fn new(exchange_registry: Arc<ExchangeRegistry>) -> Self {
        Self { exchange_registry, kline_engine: None }
    }

    pub fn with_kline_engine(mut self, engine: Arc<KlineEngine>) -> Self {
        self.kline_engine = Some(engine);
        self
    }
}

#[async_trait]
impl PriceProvider for ExchangePriceProvider {
    async fn get_price(&self, exchange: &str, symbol: &str) -> Option<f64> {
        if let Some(ref engine) = self.kline_engine {
            if let Some(candles) = engine.get_klines_async(exchange, symbol, Timeframe::M1).await {
                if let Some(last) = candles.last() {
                    if last.close > 0.0 {
                        return Some(last.close);
                    }
                }
            }
        }

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

    async fn get_account_balance(&self, exchange: &str) -> super::ports::AccountBalance {
        use super::ports::AccountBalance;
        
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
            OrderCommand::PlaceOrder { symbol, side, amount, price, reduce_only, position_side, client_order_id } => {
                let pe_position_side = position_side.map(|ps| match ps {
                    crate::trading::ports::PositionSide::Long => pe_types::PositionSide::Long,
                    crate::trading::ports::PositionSide::Short => pe_types::PositionSide::Short,
                });
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
                        position_side: pe_position_side,
                        position_id: None,
                        client_order_id,
                    },
                }
            }
            OrderCommand::CancelOrder { order_id, symbol: _ } => {
                pe_types::EngineCommand::CancelOrder { order_id }
            }
            OrderCommand::CancelAllOrders { symbol } => {
                pe_types::EngineCommand::CancelAllOrders {
                    position_id: None,
                    symbol,
                }
            }
            OrderCommand::CloseAllPositions { symbol, exchange } => {
                let exchange_key = format!("{}:perpetual", exchange);
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
        grid_levels_json: Option<&serde_json::Value>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"UPDATE qd_grid_bots SET
                market_regime = $1, upper_price = $2, lower_price = $3,
                grid_count = $4, grid_profit_pct = $5, quantity_per_grid = $6,
                leverage = $7, ai_analysis = $8, grid_levels_json = $9::jsonb,
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

/** 将 Position Engine 的 EngineEvent 转换为 OrderEvent */
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

/** 可切换的订单执行器

根据 paper 模式开关，将命令转发到真实执行器或 Paper 执行器。 */
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

