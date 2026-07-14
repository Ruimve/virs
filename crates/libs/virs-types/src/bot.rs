use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use virs_error::BotResult;

use crate::position::Position;


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


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BotPositionSide {
    Long,
    Short,
}


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

    pub fee: f64,
}


#[derive(Debug, Clone)]
pub enum OrderCommand {
    OpenPosition {
        symbol: String,
        side: BotPositionSide,
        order_side: OrderSide,
        amount: f64,
        leverage: u32,
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


#[async_trait]
pub trait OrderExecutor: Send + Sync {
    async fn send_command(&self, command: OrderCommand) -> BotResult<()>;


    async fn query_open_position(&self, symbol: &str) -> BotResult<Option<Position>>;
}


#[derive(Debug, Clone, Default)]
pub struct AccountBalance {
    pub total: f64,
    pub free: f64,
    pub used: f64,
}


#[async_trait]
pub trait CredentialStore: Send + Sync {
    async fn load_credentials(
        &self,
        user_id: Uuid,
    ) -> BotResult<Vec<(String, String, Option<String>)>>;
}


#[async_trait]
pub trait PriceProvider: Send + Sync {
    async fn get_price(&self, exchange: &str, symbol: &str) -> Option<f64>;
}


#[derive(Debug, Clone, Default)]
pub struct MarketSnapshot {
    pub current_price: f64,
    pub funding_rate: f64,
    pub funding_next_time: String,
    pub min_qty: f64,
    pub liquidation_price: Option<f64>,

    pub indicators_json: serde_json::Value,
}


#[async_trait]
pub trait MarketDataProvider: Send + Sync {
    async fn get_market_snapshot(
        &self,
        exchange: &str,
        symbol: &str,
    ) -> MarketSnapshot;
    async fn get_account_balance(&self, exchange: &str) -> AccountBalance;
}


pub trait LlmProviderResolver: Send + Sync {
    fn is_available(&self) -> bool;
    fn resolve(
        &self,
        user_credentials: &[(String, String, Option<String>)],
    ) -> BotResult<(String, String, String, String)>;
}
