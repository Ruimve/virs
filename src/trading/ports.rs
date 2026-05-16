use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

#[derive(Debug, Clone)]
pub enum OrderCommand {
    PlaceOrder {
        symbol: String,
        side: OrderSide,
        amount: f64,
        price: Option<f64>,
        reduce_only: bool,
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

#[async_trait]
pub trait OrderExecutor: Send + Sync {
    async fn send_command(&self, command: OrderCommand) -> anyhow::Result<()>;
}
