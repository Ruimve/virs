use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, error};

use crate::api::ws;
use crate::config::NotificationConfig;
use crate::engine::strategy::{OrderCommand, OrderResult, StrategyEngine};
use crate::services;

/// Order Worker: processes order commands (Place, Cancel, Query, Shutdown)
/// from the strategy engine and executes them via exchange connections.
pub struct OrderWorker {
    db_pool: sqlx::PgPool,
    strategy_engine: Arc<StrategyEngine>,
    notification_config: NotificationConfig,
    ws_broadcaster: Arc<ws::WsBroadcaster>,
}

impl OrderWorker {
    pub fn new(
        db_pool: sqlx::PgPool,
        strategy_engine: Arc<StrategyEngine>,
        notification_config: NotificationConfig,
        ws_broadcaster: Arc<ws::WsBroadcaster>,
    ) -> Self {
        Self {
            db_pool,
            strategy_engine,
            notification_config,
            ws_broadcaster,
        }
    }

    /// Run the order worker loop, processing commands until Shutdown is received.
    pub async fn run(mut self, mut order_rx: mpsc::Receiver<OrderCommand>) {
        while let Some(cmd) = order_rx.recv().await {
            match cmd {
                OrderCommand::Place {
                    strategy_id,
                    symbol,
                    signal_type,
                    side,
                    amount,
                    price,
                    order_type,
                    exchange_name,
                    market_type,
                    reduce_only,
                    position_side,
                    callback,
                } => {
                    self.handle_place_order(
                        strategy_id,
                        symbol,
                        signal_type,
                        side,
                        amount,
                        price,
                        order_type,
                        exchange_name,
                        market_type,
                        reduce_only,
                        position_side,
                        callback,
                    )
                    .await;
                }
                OrderCommand::Cancel {
                    strategy_id,
                    symbol,
                    order_id,
                    exchange_name,
                } => {
                    self.handle_cancel_order(strategy_id, symbol, order_id, exchange_name)
                        .await;
                }
                OrderCommand::Query {
                    symbol,
                    order_id,
                    exchange_name,
                    callback,
                } => {
                    self.handle_query_order(symbol, order_id, exchange_name, callback)
                        .await;
                }
                OrderCommand::Shutdown => {
                    info!("Order worker shutting down");
                    break;
                }
            }
        }
    }

    async fn handle_place_order(
        &self,
        strategy_id: uuid::Uuid,
        symbol: String,
        signal_type: crate::models::SignalType,
        side: crate::models::Side,
        amount: f64,
        price: Option<f64>,
        order_type: crate::models::OrderType,
        exchange_name: String,
        market_type: crate::models::MarketType,
        reduce_only: bool,
        position_side: Option<crate::models::PositionSide>,
        callback: tokio::sync::oneshot::Sender<OrderResult>,
    ) {
        info!(
            "Processing order: strategy={}, symbol={}, signal={:?}, side={:?}, amount={}, exchange={}, market_type={:?}",
            strategy_id, symbol, signal_type, side, amount, exchange_name, market_type
        );

        if amount <= 0.0 {
            error!("Invalid order amount {} for strategy {}. Refusing.", amount, strategy_id);
            let _ = callback.send(OrderResult::Failed {
                error: format!("Invalid order amount: {}", amount),
            });
            return;
        }

        // Insert into pending_orders table for tracking
        let _ = sqlx::query(
            r#"INSERT INTO pending_orders
               (strategy_id, symbol, signal_type, order_type, side, amount, price, status, priority, attempts, max_attempts, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, 'dispatched', 0, 0, 3, NOW(), NOW())"#,
        )
        .bind(strategy_id)
        .bind(&symbol)
        .bind(format!("{:?}", signal_type))
        .bind(format!("{:?}", order_type))
        .bind(format!("{:?}", side))
        .bind(amount)
        .bind(price)
        .execute(&self.db_pool)
        .await;

        let exchange = self.strategy_engine.get_exchange(&exchange_name);
        match exchange {
            Some(ex) => {
                match ex.place_order_with_options(&symbol, side.clone(), order_type.clone(), amount, price, Some(reduce_only), position_side.clone()).await {
                    Ok(order) => {
                        info!("✅ Order executed: id={}, symbol={}, side={:?}, status={:?}", order.id, order.symbol, order.side, order.status);

                        let fill_price = order.cost
                            .filter(|c| *c > 0.0 && order.filled > 0.0)
                            .map(|c| c / order.filled)
                            .or(order.price)
                            .unwrap_or(0.0);
                        let filled_amount = order.filled;
                        let fee = order.fee;

                        // Record trade
                        let _ = sqlx::query(
                            r#"INSERT INTO qd_strategy_trades
                               (strategy_id, symbol, side, trade_type, price, amount, fee, pnl, exchange_order_id, created_at)
                               VALUES ($1, $2, $3, $4, $5, $6, $7, 0, $8, NOW())"#,
                        )
                        .bind(strategy_id)
                        .bind(&symbol)
                        .bind(format!("{:?}", side))
                        .bind(format!("{:?}", signal_type))
                        .bind(fill_price)
                        .bind(filled_amount)
                        .bind(fee)
                        .bind(&order.id)
                        .execute(&self.db_pool)
                        .await;

                        // Send notification
                        services::notification::send_notification(
                            &self.notification_config,
                            &format!("Order Executed: {}", symbol),
                            &format!("Side: {:?}\nAmount: {}\nPrice: {:?}\nOrder ID: {}", side, filled_amount, fill_price, order.id),
                        ).await;

                        // Emit WebSocket event: order filled
                        let _ = self.ws_broadcaster.send(ws::WsEvent::Order {
                            order_id: order.id.clone(),
                            strategy_id: strategy_id.to_string(),
                            symbol: symbol.clone(),
                            status: "filled".to_string(),
                            error: None,
                        });

                        // Update pending order status to filled
                        let _ = sqlx::query(
                            "UPDATE pending_orders SET status = 'filled', exchange_order_id = $1, updated_at = NOW() WHERE strategy_id = $2 AND symbol = $3 AND status = 'dispatched'"
                        )
                        .bind(&order.id)
                        .bind(strategy_id)
                        .bind(&symbol)
                        .execute(&self.db_pool)
                        .await;

                        let _ = callback.send(OrderResult::Filled {
                            order_id: order.id,
                            fill_price,
                            filled_amount,
                            fee,
                        });
                    }
                    Err(e) => {
                        error!("❌ Order failed: strategy={}, symbol={}, error={}", strategy_id, symbol, e);

                        // Update pending order status
                        let _ = sqlx::query(
                            "UPDATE pending_orders SET status = 'failed', error_message = $1, updated_at = NOW() WHERE strategy_id = $2 AND symbol = $3 AND status = 'dispatched'"
                        )
                        .bind(e.to_string())
                        .bind(strategy_id)
                        .bind(&symbol)
                        .execute(&self.db_pool)
                        .await;

                        // Emit WebSocket event: order failed
                        let _ = self.ws_broadcaster.send(ws::WsEvent::Order {
                            order_id: String::new(),
                            strategy_id: strategy_id.to_string(),
                            symbol: symbol.clone(),
                            status: "failed".to_string(),
                            error: Some(e.to_string()),
                        });

                        let _ = callback.send(OrderResult::Failed {
                            error: e.to_string(),
                        });
                    }
                }
            }
            None => {
                error!("❌ Exchange '{}' not found for strategy {}", exchange_name, strategy_id);
                let _ = callback.send(OrderResult::Failed {
                    error: format!("Exchange '{}' not found", exchange_name),
                });
            }
        }
    }

    async fn handle_cancel_order(
        &self,
        strategy_id: uuid::Uuid,
        symbol: String,
        order_id: String,
        exchange_name: String,
    ) {
        info!("🔄 Canceling order: strategy={}, order={}, symbol={}, exchange={}", strategy_id, order_id, symbol, exchange_name);

        let exchange = self.strategy_engine.get_exchange(&exchange_name);
        match exchange {
            Some(ex) => {
                match ex.cancel_order(&symbol, &order_id).await {
                    Ok(order) => {
                        info!("✅ Order canceled: id={}, status={:?}", order.id, order.status);
                    }
                    Err(e) => {
                        error!("❌ Cancel failed: order={}, error={}", order_id, e);
                    }
                }
            }
            None => {
                error!("❌ Exchange '{}' not found for cancel", exchange_name);
            }
        }
    }

    async fn handle_query_order(
        &self,
        symbol: String,
        order_id: String,
        exchange_name: String,
        callback: tokio::sync::oneshot::Sender<OrderResult>,
    ) {
        let exchange = self.strategy_engine.get_exchange(&exchange_name);
        match exchange {
            Some(ex) => {
                match ex.get_order(&symbol, &order_id).await {
                    Ok(order) => {
                        let fill_price = order.cost
                            .filter(|c| *c > 0.0 && order.filled > 0.0)
                            .map(|c| c / order.filled)
                            .or(order.price)
                            .unwrap_or(0.0);
                        let _ = callback.send(OrderResult::Filled {
                            order_id: order.id,
                            fill_price,
                            filled_amount: order.filled,
                            fee: order.fee,
                        });
                    }
                    Err(e) => {
                        let _ = callback.send(OrderResult::Failed {
                            error: e.to_string(),
                        });
                    }
                }
            }
            None => {
                let _ = callback.send(OrderResult::Failed {
                    error: format!("Exchange '{}' not found", exchange_name),
                });
            }
        }
    }
}
