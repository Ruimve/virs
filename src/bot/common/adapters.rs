use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;
use tokio::sync::broadcast;
use tracing::{info, warn};
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
//
// 交易执行器，通过 PositionEngine 执行订单：
// - 发送 EngineCommand 到 PositionEngine
// - 监听 PositionEngine 的 EngineEvent，转换为 OrderEvent 广播
// - PositionEngine 内部通过 Exchange trait 与真实交易所或 Paper 交互
// - Paper 开关在创建 PositionEngine 时决定传入哪个 Exchange 实现

pub struct PeOrderExecutor {
    cmd_tx: tokio::sync::mpsc::Sender<pe_types::EngineCommand>,
}

impl PeOrderExecutor {
    pub fn new(
        cmd_tx: tokio::sync::mpsc::Sender<pe_types::EngineCommand>,
        event_tx: broadcast::Sender<OrderEvent>,
        mut engine_event_rx: tokio::sync::broadcast::Receiver<pe_types::EngineEvent>,
    ) -> Self {
        // 启动事件转发：EngineEvent → OrderEvent
        tokio::spawn(async move {
            loop {
                match engine_event_rx.recv().await {
                    Ok(engine_event) => {
                        if let Some(order_event) = convert_pe_event(engine_event) {
                            let _ = event_tx.send(order_event);
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!(lagged = n, "PeOrderExecutor: EngineEvent lagged");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        info!("PeOrderExecutor: EngineEvent channel closed");
                        break;
                    }
                }
            }
        });

        Self {
            cmd_tx,
        }
    }
}

#[async_trait]
impl OrderExecutor for PeOrderExecutor {
    async fn send_command(&self, command: OrderCommand) -> anyhow::Result<()> {
        let engine_cmd = match command {
            OrderCommand::PlaceOrder { symbol, side, amount, price, reduce_only, position_side, client_order_id } => {
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
                        position_side: position_side.map(|ps| match ps {
                            PositionSide::Long => pe_types::PositionSide::Long,
                            PositionSide::Short => pe_types::PositionSide::Short,
                        }),
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
            OrderCommand::CloseAllPositions { symbol, exchange: _ } => {
                pe_types::EngineCommand::CloseAllPositions { symbol }
            }
        };

        self.cmd_tx.send(engine_cmd).await
            .map_err(|e| anyhow::anyhow!("Failed to send command to PositionEngine: {}", e))
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
