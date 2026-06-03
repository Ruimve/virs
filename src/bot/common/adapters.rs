use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;
use tracing::warn;
use uuid::Uuid;

use crate::config::AiConfig;
use crate::engine::kline::KlineEngine;
use crate::engine::kline::types::{Candle, Timeframe};
use crate::engine::position::types as pe_types;
use crate::models::Kline;
use crate::services::ai::{AiService, AiUserConfig};
use crate::trading::exchange::registry::ExchangeRegistry;
use crate::trading::ports::{OrderCommand, OrderEvent, OrderExecutor, OrderInfo, OrderSide, PositionSide};

use super::ports::{CredentialStore, LlmProviderResolver};

// ── Helper ──

pub fn candle_to_kline(c: &Candle) -> Kline {
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

// ── ExchangePriceProvider ──

pub struct ExchangePriceProvider {
    exchange_registry: Arc<ExchangeRegistry>,
    kline_engine: Option<Arc<KlineEngine>>,
    market_type: String,
}

impl ExchangePriceProvider {
    pub fn new(exchange_registry: Arc<ExchangeRegistry>, market_type: &str) -> Self {
        Self { exchange_registry, kline_engine: None, market_type: market_type.to_string() }
    }

    pub fn with_kline_engine(mut self, engine: Arc<KlineEngine>) -> Self {
        self.kline_engine = Some(engine);
        self
    }

    pub async fn get_price(&self, exchange: &str, symbol: &str) -> Option<f64> {
        if let Some(ref engine) = self.kline_engine {
            if let Some(candles) = engine.get_klines_async(exchange, symbol, Timeframe::M1).await {
                if let Some(last) = candles.last() {
                    if last.close > 0.0 {
                        return Some(last.close);
                    }
                }
            }
        }

        let exchange_key = format!("{}:{}", exchange, self.market_type);
        let ex = self.exchange_registry.get(&exchange_key)?;
        match ex.get_ticker(symbol).await {
            Ok(ticker) if ticker.last > 0.0 => Some(ticker.last),
            _ => None,
        }
    }
}

// ── PgCredentialStore ──

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

// ── DefaultLlmResolver ──

pub struct DefaultLlmResolver {
    ai_service: AiService,
    #[allow(dead_code)]
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

// ── PeOrderExecutor ──

pub struct PeOrderExecutor {
    pe_cmd_tx: tokio::sync::mpsc::Sender<pe_types::EngineCommand>,
    exchange_registry: Arc<ExchangeRegistry>,
    close_prefix: String,
}

impl PeOrderExecutor {
    pub fn new(
        pe_cmd_tx: tokio::sync::mpsc::Sender<pe_types::EngineCommand>,
        exchange_registry: Arc<ExchangeRegistry>,
        close_prefix: &str,
    ) -> Self {
        Self { pe_cmd_tx, exchange_registry, close_prefix: close_prefix.to_string() }
    }
}

#[async_trait]
impl OrderExecutor for PeOrderExecutor {
    async fn send_command(&self, command: OrderCommand) -> anyhow::Result<()> {
        let pe_cmd = match command {
            OrderCommand::PlaceOrder { symbol, side, amount, price, reduce_only, position_side, client_order_id } => {
                let pe_position_side = position_side.map(|ps| match ps {
                    PositionSide::Long => pe_types::PositionSide::Long,
                    PositionSide::Short => pe_types::PositionSide::Short,
                });
                pe_types::EngineCommand::PlaceOrder {
                    params: pe_types::PlaceOrderParams {
                        symbol,
                        side: match side {
                            OrderSide::Buy => pe_types::Side::Buy,
                            OrderSide::Sell => pe_types::Side::Sell,
                        },
                        order_type: if price.is_some() {
                            pe_types::OrderType::Limit
                        } else {
                            pe_types::OrderType::Market
                        },
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
                                            client_order_id: Some(format!("{}:{}", self.close_prefix, symbol)),
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
                }
                return Ok(());
            }
        };
        self.pe_cmd_tx.send(pe_cmd).await?;
        Ok(())
    }
}

// ── SwitchableOrderExecutor ──

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

// ── convert_pe_event ──

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
