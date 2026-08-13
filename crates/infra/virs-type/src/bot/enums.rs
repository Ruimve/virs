use crate::order::Side;
use crate::position::PositionSide;
use serde::{Deserialize, Serialize};
use super::structs::OrderInfo;
use tokio::sync::oneshot;
use uuid::Uuid;
use virs_error::VirsResult;


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


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StrategyType {
    Chat,
    Agent,
}


/* 机器人命令枚举：启动、停止、删除机器人（删除操作通过 oneshot 返回结果） */
#[derive(Debug)]
pub enum BotCommand {
    StartBot { bot_id: Uuid },
    StopBot { bot_id: Uuid },
    DeleteBot {
        bot_id: Uuid,
        close_position: bool,
        /* oneshot 通道：删除操作完成后通过此通道返回结果 */
        response_tx: oneshot::Sender<VirsResult<()>>,
    },
}
