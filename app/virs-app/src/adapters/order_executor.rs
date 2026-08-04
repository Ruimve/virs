use async_trait::async_trait;
use tokio::sync::broadcast;
use tracing::{error, warn};

use uuid::Uuid;
use virs_error::{BotError, BotResult, VirsError};
use virs_position::PositionEngine;
use virs_task::{spawn, Stop, TaskHandle};
use virs_types::bot::{
    OrderCommand, OrderEvent, OrderExecutor, OrderInfo,
};
use virs_types::{OrderType, TimeInForce};
use virs_types::position::*;
use virs_types::CcxtOrder;

pub struct PeOrderExecutor {
    cmd_tx: tokio::sync::mpsc::Sender<EngineCommand>,
    engine: PositionEngine,
    forward_task: std::sync::Mutex<Option<TaskHandle>>,
}

impl PeOrderExecutor {
    pub async fn new(
        cmd_tx: tokio::sync::mpsc::Sender<EngineCommand>,
        event_tx: broadcast::Sender<OrderEvent>,
        mut engine_event_rx: broadcast::Receiver<EngineEvent>,
        engine: PositionEngine,
    ) -> Self {
        let handle = spawn("order_event_forward", move |stop: Stop| async move {
            loop {
                tokio::select! {
                    _ = stop.cancelled() => break,
                    result = engine_event_rx.recv() => {
                        match result {
                            Ok(engine_event) => {
                                if let Some(order_event) = convert_pe_event(engine_event) {
                                    if event_tx.send(order_event).is_err() {
                                        warn!("OrderEvent broadcast failed — no receivers, event dropped");
                                    }
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!(lagged = n, "EngineEvent lagged");
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                break;
                            }
                        }
                    }
                }
            }
        });

        Self {
            cmd_tx,
            engine,
            forward_task: std::sync::Mutex::new(Some(handle)),
        }
    }

    pub async fn stop(&self) {
        let handle = self.forward_task.lock().unwrap().take();
        if let Some(h) = handle {
            h.cancel();
            h.join_with_timeout(std::time::Duration::from_secs(5)).await;
        }
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
                client_order_id,
            } => EngineCommand::OpenPosition {
                exchange: String::new(),
                symbol,
                side,
                order_side,
                quantity: amount,
                leverage,
                order_type: if price.is_some() {
                    OrderType::Limit
                } else {
                    OrderType::Market
                },
                price,
                client_order_id,
            },
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
                    side,
                    order_type: if price.is_some() {
                        OrderType::Limit
                    } else {
                        OrderType::Market
                    },
                    amount,
                    price,
                    position_side,
                    position_id,
                    client_order_id,
                    stop_price: None,
                    time_in_force: Some(TimeInForce::Gtc),
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

    async fn query_open_positions(&self, symbol: &str) -> BotResult<Vec<Position>> {
        Ok(self.engine.get_open_positions_by_symbol(symbol))
    }
}

pub fn convert_pe_event(event: EngineEvent) -> Option<OrderEvent> {
    match event {
        EngineEvent::OrderPlaced { order } => ccxt_order_to_order_info(&order)
            .map(|order_info| OrderEvent::OrderPlaced { order: order_info }),
        EngineEvent::OrderFilled { order, .. } => ccxt_order_to_order_info(&order)
            .map(|order_info| OrderEvent::OrderFilled { order: order_info }),
        EngineEvent::OrderPartiallyFilled { order, .. } => ccxt_order_to_order_info(&order)
            .map(|order_info| OrderEvent::OrderPartiallyFilled { order: order_info }),
        EngineEvent::OrderCanceled { order } => Some(OrderEvent::OrderCanceled {
            order_id: Uuid::from_u128(order.order_id as u128),
            client_order_id: Some(order.client_order_id.clone()),
            symbol: Some(order.symbol.clone()),
        }),
        EngineEvent::OrderFailed {
            client_order_id,
            reason,
        } => Some(OrderEvent::OrderFailed {
            order_id: Uuid::new_v4(),
            client_order_id: Some(client_order_id),
            reason,
        }),
        EngineEvent::RiskAlert { level, message } => Some(OrderEvent::RiskAlert { level, message }),
        _ => None,
    }
}

fn ccxt_order_to_order_info(order: &CcxtOrder) -> Option<OrderInfo> {
    let filled = order.filled_qty.parse::<f64>().map_err(|_| {
        error!(
            order_id = %order.order_id,
            symbol = %order.symbol,
            filled_qty = %order.filled_qty,
            error = %VirsError::bad_request("Failed to parse filled_qty as f64"),
            "Skipping order — malformed filled_qty from exchange"
        );
    }).ok()?;
    let fee = order.commission.parse::<f64>().map_err(|_| {
        error!(
            order_id = %order.order_id,
            symbol = %order.symbol,
            commission = %order.commission,
            error = %VirsError::bad_request("Failed to parse commission as f64"),
            "Skipping order — malformed commission from exchange"
        );
    }).ok()?;
    Some(OrderInfo {
        id: Uuid::from_u128(order.order_id as u128),
        position_id: None,
        symbol: order.symbol.clone(),
        side: order.side.clone(),
        fill_price: order.avg_fill_price.parse::<f64>().ok(),
        request_price: order.original_price.parse::<f64>().ok(),
        filled,
        client_order_id: Some(order.client_order_id.clone()),
        fee,
    })
}
