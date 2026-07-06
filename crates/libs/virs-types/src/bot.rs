//! Bot-layer types: OrderSide, OrderCommand, OrderEvent, OrderExecutor, etc.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use virs_error::BotResult;

use crate::position::Position;

/// Order side (bot-layer, distinct from engine Side for domain separation)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

impl OrderSide {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
        }
    }
}

/// Position side for bot layer (Long/Short only, no Both)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BotPositionSide {
    Long,
    Short,
}

/// Order info (bot-layer)
#[derive(Debug, Clone)]
pub struct OrderInfo {
    pub id: Uuid,
    pub position_id: Option<Uuid>,
    pub symbol: String,
    pub side: OrderSide,
    pub fill_price: Option<f64>,
    pub request_price: Option<f64>,
    pub filled: f64,
    pub client_order_id: Option<String>,
    /// 本次成交手续费（计价货币）
    pub fee: f64,
}

/// Order command (bot-layer)
#[derive(Debug, Clone)]
pub enum OrderCommand {
    OpenPosition {
        symbol: String,
        side: BotPositionSide,
        order_side: OrderSide,
        amount: f64,
        leverage: Option<u32>,
        price: Option<f64>,
        stop_loss: Option<f64>,
        take_profit: Option<f64>,
        client_order_id: Option<String>,
    },
    ClosePosition {
        position_id: Uuid,
        price: Option<f64>,
        client_order_id: Option<String>,
    },
    PlaceOrder {
        symbol: String,
        side: OrderSide,
        amount: f64,
        price: Option<f64>,
        reduce_only: bool,
        position_side: Option<BotPositionSide>,
        position_id: Option<Uuid>,
        client_order_id: Option<String>,
    },
    CancelOrder {
        order_id: Uuid,
        symbol: String,
    },
    CancelAllOrders {
        symbol: Option<String>,
    },
    CloseAllPositions {
        symbol: String,
        exchange: String,
    },
}

/// Order event (bot-layer)
#[derive(Debug, Clone)]
pub enum OrderEvent {
    OrderPlaced {
        order: OrderInfo,
    },
    OrderFilled {
        order: OrderInfo,
    },
    OrderCanceled {
        order_id: Uuid,
        symbol: Option<String>,
    },
    OrderFailed {
        order_id: Uuid,
        reason: String,
    },
    RiskAlert {
        level: String,
        message: String,
    },
}

/// Order executor trait (bot-layer)
#[async_trait]
pub trait OrderExecutor: Send + Sync {
    async fn send_command(&self, command: OrderCommand) -> BotResult<()>;

    /// 直接查询 PE 当前 Open 仓位（按 symbol），用于 LLM 决策前刷新缓存，防止事件丢失导致重复开仓。
    async fn query_open_position(&self, symbol: &str) -> BotResult<Option<Position>>;
}

/// Account balance (bot-layer)
#[derive(Debug, Clone, Default)]
pub struct AccountBalance {
    pub total: f64,
    pub free: f64,
    pub used: f64,
}

/// Credential store trait
#[async_trait]
pub trait CredentialStore: Send + Sync {
    async fn load_credentials(
        &self,
        user_id: Uuid,
    ) -> BotResult<Vec<(String, String, Option<String>)>>;
}

/// Price provider trait (unified — market_type defaults to "perpetual" for grid)
#[async_trait]
pub trait PriceProvider: Send + Sync {
    async fn get_price(&self, exchange: &str, symbol: &str, market_type: &str) -> Option<f64>;
}

/// Market snapshot (unified — grid uses subset, auto uses full)
#[derive(Debug, Clone, Default)]
pub struct MarketSnapshot {
    pub current_price: f64,
    pub funding_rate: f64,
    pub funding_next_time: String,
    pub min_qty: f64,
    pub liquidation_price: Option<f64>,
    /// Serialized MarketIndicators from virs-bot (opaque to virs-types)
    pub indicators_json: serde_json::Value,
}

/// Market data provider trait (unified — market_type parameter for auto, grid passes "perpetual")
#[async_trait]
pub trait MarketDataProvider: Send + Sync {
    async fn get_market_snapshot(
        &self,
        exchange: &str,
        symbol: &str,
        market_type: &str,
    ) -> MarketSnapshot;
    async fn get_account_balance(&self, exchange: &str, market_type: &str) -> AccountBalance;
}

/// LLM provider resolver trait
pub trait LlmProviderResolver: Send + Sync {
    fn is_available(&self) -> bool;
    fn resolve(
        &self,
        user_credentials: &[(String, String, Option<String>)],
    ) -> BotResult<(String, String, String, String)>;
}
