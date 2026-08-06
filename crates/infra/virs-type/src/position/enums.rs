use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::order::CcxtOrder;
use super::structs::{Position, Trade};


#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PositionSide {
    #[serde(rename = "LONG")]
    Long,
    #[serde(rename = "SHORT")]
    Short,
    #[serde(untagged)]
    Unknown(String),
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PositionStatus {
    Opening,
    Open,
    Closing,
    Closed,
}

impl PositionStatus {
    pub fn is_open(&self) -> bool {
        self == &Self::Open
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EngineState {
    Created,
    Running,
    ShuttingDown,
    Stopped,
}

impl EngineState {
    pub fn is_running(&self) -> bool {
        self == &Self::Running
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeType {
    Open,
    Close,
}


// ── sqlx 编解码：PositionSide ──

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for PositionSide {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <&str as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        match s {
            "long" => Ok(PositionSide::Long),
            "short" => Ok(PositionSide::Short),
            other => Ok(PositionSide::Unknown(other.to_string())),
        }
    }
}

impl sqlx::Type<sqlx::Postgres> for PositionSide {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("text")
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Postgres> for PositionSide {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        let s = match self {
            PositionSide::Long => "long",
            PositionSide::Short => "short",
            PositionSide::Unknown(other) => other.as_str(),
        };
        <&str as sqlx::Encode<sqlx::Postgres>>::encode_by_ref(&s, buf)
    }
}


// ── 引擎通信枚举 ──

#[derive(Debug, Clone, PartialEq)]
pub enum WsFeedEvent {
    OrderUpdate {
        order: Arc<CcxtOrder>,
    },
    ConnectionChanged {
        connected: bool,
    },
}


#[derive(Debug, Clone)]
pub enum EngineCommand {
    OpenPosition {
        exchange: String,
        symbol: String,
        side: PositionSide,
        order_side: crate::order::Side,
        quantity: f64,
        leverage: u32,
        order_type: crate::order::OrderType,
        price: Option<f64>,
        client_order_id: Option<String>,
    },
    ClosePosition {
        position_id: uuid::Uuid,
        order_type: crate::order::OrderType,
        price: Option<f64>,
        client_order_id: Option<String>,
    },
    PlaceOrder {
        params: super::structs::PlaceOrderParams,
    },
    CancelAllOrders {
        position_id: Option<uuid::Uuid>,
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
    PositionUpdated {
        position: Position,
    },
    OrderPlaced {
        order: Arc<CcxtOrder>,
    },
    OrderFilled {
        order: Arc<CcxtOrder>,
        trade: Trade,
    },
    OrderPartiallyFilled {
        order: Arc<CcxtOrder>,
        trade: Trade,
    },
    OrderCanceled {
        order: Arc<CcxtOrder>,
    },
    OrderFailed {
        client_order_id: String,
        reason: String,
    },
    RiskAlert {
        level: String,
        message: String,
    },
}
