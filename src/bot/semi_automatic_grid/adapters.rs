//! Grid Engine 的 Adapter 实现
//!
//! 将外部模块（StrategyEngine, Position Engine, Database, AiService 等）
//! 适配为 semi_automatic_grid 模块定义的 trait 接口。

use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::bot::semi_automatic_grid::ports::*;
use crate::config::AiConfig;
use crate::engine::strategy::StrategyEngine;
use crate::engine::position::types as pe_types;
use crate::services::ai::{AiService, AiUserConfig};

// ── PriceProvider ──

pub struct StrategyPriceProvider {
    strategy_engine: Arc<StrategyEngine>,
}

impl StrategyPriceProvider {
    pub fn new(strategy_engine: Arc<StrategyEngine>) -> Self {
        Self { strategy_engine }
    }
}

#[async_trait]
impl PriceProvider for StrategyPriceProvider {
    async fn get_price(&self, exchange: &str, symbol: &str) -> Option<f64> {
        let exchange_key = format!("{}:Perpetual", exchange);
        let ex = self.strategy_engine.get_exchange(&exchange_key)?;
        match ex.get_ticker(symbol).await {
            Ok(ticker) if ticker.last > 0.0 => Some(ticker.last),
            _ => None,
        }
    }
}

// ── OrderExecutor ──

pub struct PeOrderExecutor {
    pe_cmd_tx: tokio::sync::mpsc::Sender<pe_types::EngineCommand>,
}

impl PeOrderExecutor {
    pub fn new(pe_cmd_tx: tokio::sync::mpsc::Sender<pe_types::EngineCommand>) -> Self {
        Self { pe_cmd_tx }
    }
}

#[async_trait]
impl OrderExecutor for PeOrderExecutor {
    async fn send_command(&self, command: GridOrderCommand) -> anyhow::Result<()> {
        let pe_cmd = match command {
            GridOrderCommand::PlaceOrder { symbol, side, amount, price, reduce_only } => {
                pe_types::EngineCommand::PlaceOrder {
                    params: pe_types::PlaceOrderParams {
                        symbol,
                        side: match side {
                            GridSide::Buy => pe_types::Side::Buy,
                            GridSide::Sell => pe_types::Side::Sell,
                        },
                        order_type: pe_types::OrderType::Limit,
                        amount,
                        price,
                        reduce_only,
                        position_side: Some(pe_types::PositionSide::Long),
                    },
                }
            }
            GridOrderCommand::CancelAllOrders { symbol } => {
                pe_types::EngineCommand::CancelAllOrders {
                    position_id: None,
                    symbol,
                }
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
        dynamic_adjust: bot.dynamic_adjust,
        adjust_interval_secs: bot.adjust_interval_secs,
        market_regime: bot.market_regime.clone(),
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

/// 将 Position Engine 的 EngineEvent 转换为 GridOrderEvent
pub fn convert_pe_event(event: pe_types::EngineEvent) -> Option<GridOrderEvent> {
    match event {
        pe_types::EngineEvent::OrderPlaced { order } => Some(GridOrderEvent::OrderPlaced {
            order: GridOrderInfo {
                id: order.id,
                side: match order.side {
                    pe_types::Side::Buy => GridSide::Buy,
                    pe_types::Side::Sell => GridSide::Sell,
                },
                fill_price: order.fill_price,
                request_price: order.request_price,
                filled: order.filled,
            },
        }),
        pe_types::EngineEvent::OrderFilled { order, trade: _ } => Some(GridOrderEvent::OrderFilled {
            order: GridOrderInfo {
                id: order.id,
                side: match order.side {
                    pe_types::Side::Buy => GridSide::Buy,
                    pe_types::Side::Sell => GridSide::Sell,
                },
                fill_price: order.fill_price,
                request_price: order.request_price,
                filled: order.filled,
            },
        }),
        pe_types::EngineEvent::OrderCanceled { order } => Some(GridOrderEvent::OrderCanceled {
            order_id: order.id,
        }),
        pe_types::EngineEvent::OrderFailed { order_id, reason } => Some(GridOrderEvent::OrderFailed {
            order_id,
            reason,
        }),
        pe_types::EngineEvent::RiskAlert { level, message } => Some(GridOrderEvent::RiskAlert { level, message }),
        pe_types::EngineEvent::LiquidationWarning { symbol, liquidation_price, current_price, .. } => {
            Some(GridOrderEvent::LiquidationWarning { symbol, liquidation_price, current_price })
        }
        _ => None,
    }
}
