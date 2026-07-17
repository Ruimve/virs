use async_trait::async_trait;
use tokio::sync::broadcast;
use tracing::warn;

use virs_position::PositionEngine;
use virs_types::bot::{
    BotPositionSide, OrderCommand, OrderEvent, OrderExecutor, OrderInfo,
    OrderSide,
};
use virs_error::{BotError, BotResult};
use virs_types::enums::{OrderType, PositionSide, Side};
use virs_types::position::*;
use virs_types::CcxtOrder;
use uuid::Uuid;

pub struct PeOrderExecutor {
    cmd_tx: tokio::sync::mpsc::Sender<EngineCommand>,
    engine: PositionEngine,
}

impl PeOrderExecutor {
    pub fn new(
        cmd_tx: tokio::sync::mpsc::Sender<EngineCommand>,
        event_tx: broadcast::Sender<OrderEvent>,
        mut engine_event_rx: broadcast::Receiver<EngineEvent>,
        engine: PositionEngine,
    ) -> Self {

        tokio::spawn(async move {
            loop {
                match engine_event_rx.recv().await {
                    Ok(engine_event) => {
                        if let Some(order_event) = convert_pe_event(engine_event) {
                            if event_tx.send(order_event).is_err() {
                                warn!("OrderEvent broadcast failed — no receivers, event dropped");
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!(lagged = n, "PeOrderExecutor: EngineEvent lagged");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        });

        Self { cmd_tx, engine }
    }
}

#[async_trait]
impl OrderExecutor for PeOrderExecutor {
    async fn send_command(&self, command: OrderCommand) -> BotResult<()> {
        let engine_cmd = match command {
            OrderCommand::OpenPosition {
                symbol,
                side,
                order_side,
                amount,
                leverage,
                price,
                stop_loss,
                take_profit,
                client_order_id,
            } => {


                EngineCommand::OpenPosition {
                    exchange: String::new(),
                    symbol,
                    side: match side {
                        BotPositionSide::Long => PositionSide::Long,
                        BotPositionSide::Short => PositionSide::Short,
                    },
                    order_side: match order_side {
                        OrderSide::Buy => Side::Buy,
                        OrderSide::Sell => Side::Sell,
                    },
                    quantity: amount,
                    leverage,
                    order_type: if price.is_some() {
                        OrderType::Limit
                    } else {
                        OrderType::Market
                    },
                    price,
                    stop_loss,
                    take_profit,
                    client_order_id,
                }
            }
            OrderCommand::ClosePosition {
                position_id,
                price,
                client_order_id,
            } => EngineCommand::ClosePosition {
                position_id,
                order_type: if price.is_some() {
                    OrderType::Limit
                } else {
                    OrderType::Market
                },
                price,
                client_order_id,
            },
            OrderCommand::PlaceOrder {
                symbol,
                side,
                amount,
                price,
                position_side,
                position_id,
                client_order_id,
            } => EngineCommand::PlaceOrder {
                params: PlaceOrderParams {
                    symbol,
                    side: match side {
                        OrderSide::Buy => Side::Buy,
                        OrderSide::Sell => Side::Sell,
                    },
                    order_type: if price.is_some() {
                        OrderType::Limit
                    } else {
                        OrderType::Market
                    },
                    amount,
                    price,
                    position_side: position_side.map(|ps| match ps {
                        BotPositionSide::Long => PositionSide::Long,
                        BotPositionSide::Short => PositionSide::Short,
                    }),
                    position_id,
                    client_order_id,
                },
            },
            OrderCommand::CancelAllOrders { symbol } => EngineCommand::CancelAllOrders {
                position_id: None,
                symbol,
            },
            OrderCommand::CloseAllPositions {
                symbol,
                exchange: _,
            } => EngineCommand::CloseAllPositions { symbol },
        };

        self.cmd_tx.send(engine_cmd).await.map_err(|e| {
            BotError::OrderExecution(format!("Failed to send command to PositionEngine: {}", e))
        })
    }

    async fn query_open_position(&self, symbol: &str) -> BotResult<Option<Position>> {
        Ok(self.engine.get_open_position_by_symbol(symbol))
    }
}


pub fn convert_pe_event(event: EngineEvent) -> Option<OrderEvent> {
    match event {
        EngineEvent::OrderPlaced { order } => Some(OrderEvent::OrderPlaced {
            order: ccxt_order_to_order_info(&order),
        }),
        EngineEvent::OrderFilled { order, .. } => Some(OrderEvent::OrderFilled {
            order: ccxt_order_to_order_info(&order),
        }),
        EngineEvent::OrderCanceled { order } => Some(OrderEvent::OrderCanceled {
            order_id: Uuid::from_u128(order.order_id as u128),
            symbol: Some(order.symbol.clone()),
        }),
        EngineEvent::OrderFailed { client_order_id: _, reason } => {
            Some(OrderEvent::OrderFailed {
                order_id: Uuid::new_v4(),
                reason,
            })
        }
        EngineEvent::RiskAlert { level, message } => Some(OrderEvent::RiskAlert { level, message }),
        _ => None,
    }
}

fn ccxt_order_to_order_info(order: &CcxtOrder) -> OrderInfo {
    OrderInfo {
        id: Uuid::from_u128(order.order_id as u128),
        position_id: None,
        symbol: order.symbol.clone(),
        side: match order.side {
            Side::Buy => OrderSide::Buy,
            Side::Sell => OrderSide::Sell,
        },
        fill_price: order.avg_fill_price.parse::<f64>().ok(),
        request_price: order.original_price.parse::<f64>().ok(),
        filled: order.filled_qty.parse::<f64>().unwrap_or(0.0),
        client_order_id: Some(order.client_order_id.clone()),
        fee: order.commission.parse::<f64>().unwrap_or(0.0),
    }
}
