use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::enums::*;


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub id: Uuid,
    pub strategy_id: Option<String>,
    pub exchange: String,
    pub symbol: String,
    pub side: PositionSide,
    pub status: PositionStatus,
    pub size: f64,
    pub entry_price: f64,
    pub current_price: f64,
    pub leverage: u32,
    pub margin: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub liquidation_price: Option<f64>,
    pub opened_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub metadata: serde_json::Value,
}

impl Position {
    pub fn is_open(&self) -> bool {
        self.status.is_open()
    }


    pub fn unrealized_pnl_at(&self, current_price: f64) -> f64 {
        match self.side {
            PositionSide::Long => (current_price - self.entry_price) * self.size,
            PositionSide::Short => (self.entry_price - current_price) * self.size,
        }
    }
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PositionOrder {
    pub id: Uuid,
    pub position_id: Uuid,
    pub exchange_order_id: Option<String>,
    pub client_order_id: Option<String>,
    pub exchange: String,
    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    pub request_price: Option<f64>,
    pub fill_price: Option<f64>,
    pub amount: f64,
    pub filled: f64,
    pub remaining: f64,
    pub status: OrderStatus,
    pub reduce_only: bool,
    pub fee: f64,
    pub fee_currency: String,
    pub slippage: Option<f64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trade {
    pub id: Uuid,
    pub position_id: Uuid,
    pub order_id: Uuid,
    pub exchange: String,
    pub symbol: String,
    pub side: Side,
    pub price: f64,
    pub amount: f64,
    pub fee: f64,
    pub fee_currency: String,
    pub pnl: f64,
    pub trade_type: TradeType,
    pub created_at: DateTime<Utc>,
}


#[derive(Debug, Clone, PartialEq)]
pub enum WsFeedEvent {
    OrderUpdate {

        exchange_order_id: String,

        client_order_id: Option<String>,
        symbol: String,
        status: OrderStatus,
        filled: f64,
        remaining: f64,
        price: f64,
        amount: f64,
        commission: f64,
        timestamp: DateTime<Utc>,
        position_side: Option<PositionSide>,
    },
    ConnectionChanged {
        connected: bool,
    },
}


#[derive(Debug, Clone)]
pub struct PlaceOrderParams {
    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    pub amount: f64,
    pub price: Option<f64>,
    pub reduce_only: bool,
    pub position_side: Option<PositionSide>,
    pub position_id: Option<Uuid>,
    pub client_order_id: Option<String>,
}


#[derive(Debug, Clone)]
pub enum EngineCommand {
    OpenPosition {
        exchange: String,
        symbol: String,
        side: PositionSide,
        order_side: Side,
        size: f64,
        leverage: u32,
        order_type: OrderType,
        price: Option<f64>,
        stop_loss: Option<f64>,
        take_profit: Option<f64>,
        strategy_id: Option<String>,
    },
    ClosePosition {
        position_id: Uuid,
        order_type: OrderType,
        price: Option<f64>,
        strategy_id: Option<String>,
    },
    PlaceOrder {
        params: PlaceOrderParams,
    },
    CancelOrder {
        order_id: Uuid,
    },
    CancelAllOrders {
        position_id: Option<Uuid>,
        symbol: Option<String>,
    },
    CloseAllPositions {
        symbol: String,
    },
    PriceTick {
        symbol: String,
        price: f64,
    },
}


#[derive(Debug, Clone)]
pub enum EngineEvent {
    PositionOpened {
        position: Position,
    },
    PositionClosed {
        position: Position,
    },
    PositionModified {
        position_id: Uuid,
        stop_loss: Option<f64>,
        take_profit: Option<f64>,
    },


    PositionUpdated {
        position: Position,
    },
    OrderPlaced {
        order: PositionOrder,
    },
    OrderFilled {
        order: PositionOrder,
        trade: Trade,
    },
    OrderPartiallyFilled {
        order: PositionOrder,
        trade: Trade,
    },
    OrderCanceled {
        order: PositionOrder,
    },
    OrderFailed {
        order_id: Uuid,
        reason: String,
    },
    RiskAlert {
        level: String,
        message: String,
    },
    PositionSynced {
        positions: Vec<crate::market::ExchangePosition>,
    },
}
