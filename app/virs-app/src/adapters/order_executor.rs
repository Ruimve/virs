//! PeOrderExecutor — executes orders through the Position Engine.

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
        // Bridge: EngineEvent → OrderEvent
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
                // 注意：bot 层的 client_order_id 映射到引擎层的 strategy_id。
                // 引擎内部会把它同时保存到 Position.strategy_id（仓位归属）
                // 和作为 client_order_id 传给交易所（订单追踪）。
                // bot 层的 client_order_id 格式如 "auto:long:{bot_id}" 包含策略信息，可同时承担两个角色。
                EngineCommand::OpenPosition {
                    exchange: String::new(), // will be resolved by engine
                    symbol,
                    side: match side {
                        BotPositionSide::Long => PositionSide::Long,
                        BotPositionSide::Short => PositionSide::Short,
                    },
                    order_side: match order_side {
                        OrderSide::Buy => Side::Buy,
                        OrderSide::Sell => Side::Sell,
                    },
                    size: amount,
                    leverage,
                    order_type: if price.is_some() {
                        OrderType::Limit
                    } else {
                        OrderType::Market
                    },
                    price,
                    stop_loss,
                    take_profit,
                    strategy_id: client_order_id,
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
                strategy_id: client_order_id,
            },
            OrderCommand::PlaceOrder {
                symbol,
                side,
                amount,
                price,
                reduce_only,
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
                    reduce_only,
                    position_side: position_side.map(|ps| match ps {
                        BotPositionSide::Long => PositionSide::Long,
                        BotPositionSide::Short => PositionSide::Short,
                    }),
                    position_id,
                    client_order_id,
                },
            },
            OrderCommand::CancelOrder {
                order_id,
                symbol: _,
            } => EngineCommand::CancelOrder { order_id },
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

/// Convert a Position Engine event to a bot-layer OrderEvent.
pub fn convert_pe_event(event: EngineEvent) -> Option<OrderEvent> {
    match event {
        EngineEvent::OrderPlaced { order } => Some(OrderEvent::OrderPlaced {
            order: OrderInfo {
                id: order.id,
                position_id: Some(order.position_id),
                symbol: order.symbol.clone(),
                side: match order.side {
                    Side::Buy => OrderSide::Buy,
                    Side::Sell => OrderSide::Sell,
                },
                fill_price: order.fill_price,
                request_price: order.request_price,
                filled: order.filled,
                client_order_id: order.client_order_id.clone(),
                fee: order.fee,
            },
        }),
        EngineEvent::OrderFilled { order, .. } => Some(OrderEvent::OrderFilled {
            order: OrderInfo {
                id: order.id,
                position_id: Some(order.position_id),
                symbol: order.symbol.clone(),
                side: match order.side {
                    Side::Buy => OrderSide::Buy,
                    Side::Sell => OrderSide::Sell,
                },
                fill_price: order.fill_price,
                request_price: order.request_price,
                filled: order.filled,
                client_order_id: order.client_order_id.clone(),
                fee: order.fee,
            },
        }),
        EngineEvent::OrderCanceled { order } => Some(OrderEvent::OrderCanceled {
            order_id: order.id,
            symbol: Some(order.symbol.clone()),
        }),
        EngineEvent::OrderFailed { order_id, reason } => {
            Some(OrderEvent::OrderFailed { order_id, reason })
        }
        EngineEvent::RiskAlert { level, message } => Some(OrderEvent::RiskAlert { level, message }),
        EngineEvent::LiquidationWarning {
            symbol,
            liquidation_price,
            current_price,
            ..
        } => Some(OrderEvent::LiquidationWarning {
            symbol,
            liquidation_price,
            current_price,
        }),
        _ => None,
    }
}
