use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GridSide {
    Buy,
    Sell,
}

impl GridSide {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
        }
    }
}

#[derive(Debug, Clone)]
pub struct GridOrderInfo {
    pub id: Uuid,
    pub side: GridSide,
    pub fill_price: Option<f64>,
    pub request_price: Option<f64>,
    pub filled: f64,
}

#[derive(Debug, Clone)]
pub enum GridOrderCommand {
    PlaceOrder {
        symbol: String,
        side: GridSide,
        amount: f64,
        price: Option<f64>,
        reduce_only: bool,
    },
    CancelAllOrders {
        symbol: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub enum GridOrderEvent {
    OrderPlaced { order: GridOrderInfo },
    OrderFilled { order: GridOrderInfo },
    OrderCanceled { order_id: Uuid },
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
    async fn send_command(&self, command: GridOrderCommand) -> anyhow::Result<()>;
}
