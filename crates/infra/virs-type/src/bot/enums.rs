use crate::order::Side;
use crate::position::PositionSide;
use super::structs::OrderInfo;


/* 交易命令枚举：开仓、平仓、下单、撤单、全平 */
#[derive(Debug, Clone)]
pub enum OrderCommand {
    OpenPosition {
        symbol: String,
        side: PositionSide,
        order_side: Side,
        amount: f64,
        leverage: u32,
        price: Option<f64>,
        client_order_id: Option<String>,
    },
    ClosePosition {
        position_id: uuid::Uuid,
        price: Option<f64>,
        client_order_id: Option<String>,
    },
    PlaceOrder {
        symbol: String,
        side: Side,
        amount: f64,
        price: Option<f64>,
        position_side: Option<PositionSide>,
        position_id: Option<uuid::Uuid>,
        client_order_id: Option<String>,
    },
    CancelAllOrders {
        symbol: Option<String>,
    },
    CloseAllPositions {
        symbol: String,
        exchange: String,
    },
}


/* 交易事件枚举：订单状态变化通知，由执行器广播给外部订阅者 */
#[derive(Debug, Clone)]
pub enum OrderEvent {
    OrderPlaced {
        order: OrderInfo,
    },
    OrderFilled {
        order: OrderInfo,
    },
    OrderPartiallyFilled {
        order: OrderInfo,
    },
    OrderCanceled {
        order_id: uuid::Uuid,
        client_order_id: Option<String>,
        symbol: Option<String>,
    },
    OrderFailed {
        order_id: uuid::Uuid,
        client_order_id: Option<String>,
        reason: String,
    },
    RiskAlert {
        level: String,
        message: String,
    },
}
