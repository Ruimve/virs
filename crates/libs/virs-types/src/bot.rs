//! Bot-layer types: OrderSide, OrderCommand, OrderEvent, OrderExecutor, etc.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Bot-layer error type
#[derive(Debug, thiserror::Error)]
pub enum BotError {
    #[error("Order execution failed: {0}")]
    OrderExecution(String),
    #[error("Credential error: {0}")]
    Credential(String),
    #[error("LLM error: {0}")]
    Llm(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Bot-layer result type
pub type BotResult<T> = std::result::Result<T, BotError>;

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
    pub symbol: String,
    pub side: OrderSide,
    pub fill_price: Option<f64>,
    pub request_price: Option<f64>,
    pub filled: f64,
    pub client_order_id: Option<String>,
}

/// Order command (bot-layer)
#[derive(Debug, Clone)]
pub enum OrderCommand {
    PlaceOrder {
        symbol: String,
        side: OrderSide,
        amount: f64,
        price: Option<f64>,
        reduce_only: bool,
        position_side: Option<BotPositionSide>,
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
    OrderPlaced { order: OrderInfo },
    OrderFilled { order: OrderInfo },
    OrderCanceled { order_id: Uuid, symbol: Option<String> },
    OrderFailed { order_id: Uuid, reason: String },
    RiskAlert { level: String, message: String },
    LiquidationWarning {
        symbol: String,
        liquidation_price: f64,
        current_price: f64,
    },
}

/// Order executor trait (bot-layer)
#[async_trait]
pub trait OrderExecutor: Send + Sync {
    async fn send_command(&self, command: OrderCommand) -> BotResult<()>;
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
    async fn load_credentials(&self, user_id: Uuid) -> BotResult<Vec<(String, String)>>;
}

/// LLM provider resolver trait
pub trait LlmProviderResolver: Send + Sync {
    fn is_available(&self) -> bool;
    fn resolve(
        &self,
        user_credentials: &[(String, String)],
    ) -> BotResult<(String, String, String, String)>;
}
