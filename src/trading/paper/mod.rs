//! Paper 交易引擎
//!
//! 实现纸面交易（模拟撮合），使用真实行情但不发送真实订单。
//! 当价格到达挂单价时自动成交，通过事件通道通知 GridWorker。

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::{broadcast, Mutex};
use tracing::{debug, info};
use uuid::Uuid;

use crate::trading::ports::*;

/// 纸面挂单
#[derive(Debug, Clone)]
struct PendingOrder {
    id: Uuid,
    symbol: String,
    side: OrderSide,
    amount: f64,
    price: f64,
    reduce_only: bool,
    client_order_id: Option<String>,
    created_at: chrono::DateTime<Utc>,
}

/// Paper 交易执行器
///
/// 实现 `OrderExecutor` trait，本地维护挂单簿。
/// 不发送真实订单，而是等待 `on_price_tick` 检查触发条件后
/// 通过 `event_tx` 发送模拟的 `OrderEvent`。
pub struct PaperOrderExecutor {
    pending: Arc<Mutex<HashMap<Uuid, PendingOrder>>>,
    event_txs: Arc<tokio::sync::RwLock<Vec<broadcast::Sender<OrderEvent>>>>,
    enabled: Arc<std::sync::atomic::AtomicBool>,
}

impl PaperOrderExecutor {
    pub fn new(event_tx: broadcast::Sender<OrderEvent>) -> Self {
        Self {
            pending: Arc::new(Mutex::new(HashMap::new())),
            event_txs: Arc::new(tokio::sync::RwLock::new(vec![event_tx])),
            enabled: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        }
    }

    pub async fn add_event_channel(&self, event_tx: broadcast::Sender<OrderEvent>) {
        let mut txs = self.event_txs.write().await;
        if !txs.iter().any(|tx| tx.same_channel(&event_tx)) {
            txs.push(event_tx);
        }
    }

    fn broadcast_event(&self, event: OrderEvent) {
        if let Ok(txs) = self.event_txs.try_read() {
            for tx in txs.iter() {
                let _ = tx.send(event.clone());
            }
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// 启用 paper 交易
    pub fn enable(&self) {
        self.enabled.store(true, std::sync::atomic::Ordering::Relaxed);
        info!("Paper trading enabled");
    }

    /// 禁用 paper 交易（同时撤销所有挂单）
    pub async fn disable(&self) {
        self.enabled.store(false, std::sync::atomic::Ordering::Relaxed);
        let mut pending = self.pending.lock().await;
        for order_id in pending.keys() {
            self.broadcast_event(OrderEvent::OrderCanceled { order_id: *order_id, symbol: None });
        }
        pending.clear();
        info!("Paper trading disabled, all pending orders canceled");
    }

    /// 价格 tick — 检查挂单是否触发成交
    ///
    /// 由外部定时调用（与 GridWorker 的 price_tick 同频或更高频）。
    /// 买单：当前价格 <= 挂单价 → 成交
    /// 卖单：当前价格 >= 挂单价 → 成交
    pub async fn on_price_tick(&self, symbol: &str, current_price: f64) {
        if !self.is_enabled() || current_price <= 0.0 {
            return;
        }

        let mut triggered = Vec::new();

        {
            let mut pending = self.pending.lock().await;
            let ids: Vec<Uuid> = pending.keys().copied().collect();

            for order_id in ids {
                if let Some(order) = pending.get(&order_id) {
                    if order.symbol != symbol {
                        continue;
                    }

                    let filled = match order.side {
                        OrderSide::Buy => current_price <= order.price,
                        OrderSide::Sell => current_price >= order.price,
                    };

                    if filled {
                        triggered.push(pending.remove(&order_id).unwrap());
                    }
                }
            }
        }

        // 在锁外发送事件，避免死锁
        for order in &triggered {
            self.emit_filled(order, current_price).await;
        }
    }

    /// 模拟成交事件
    async fn emit_filled(&self, order: &PendingOrder, fill_price: f64) {
        let order_info = OrderInfo {
            id: order.id,
            symbol: order.symbol.clone(),
            side: order.side,
            fill_price: Some(fill_price),
            request_price: Some(order.price),
            filled: order.amount,
            client_order_id: order.client_order_id.clone(),
        };

        // 先发送 OrderPlaced（如果 worker 之前没收到的话）
        self.broadcast_event(OrderEvent::OrderPlaced {
            order: order_info.clone(),
        });

        self.broadcast_event(OrderEvent::OrderFilled {
            order: order_info,
        });

        debug!(
            order_id = %order.id,
            symbol = %order.symbol,
            side = ?order.side,
            price = order.price,
            fill_price,
            amount = order.amount,
            "Paper order filled"
        );
    }

    /// 获取当前挂单数量
    pub async fn pending_count(&self) -> usize {
        self.pending.lock().await.len()
    }

    /// 获取所有挂单信息
    pub async fn get_pending_orders(&self) -> Vec<PendingOrder> {
        self.pending.lock().await.values().cloned().collect()
    }
}

#[async_trait]
impl OrderExecutor for PaperOrderExecutor {
    async fn send_command(&self, command: OrderCommand) -> anyhow::Result<()> {
        if !self.is_enabled() {
            anyhow::bail!("Paper trading is disabled");
        }

        match command {
            OrderCommand::PlaceOrder {
                symbol,
                side,
                amount,
                price,
                reduce_only,
                position_side: _,
                client_order_id,
            } => {
                if price.is_none() {
                    // Market order: fill immediately, fill_price=None 让 worker 用 current_price 填充
                    let order_id = Uuid::new_v4();

                    let order_info = OrderInfo {
                        id: order_id,
                        symbol: symbol.clone(),
                        side,
                        fill_price: None,
                        request_price: None,
                        filled: amount,
                        client_order_id: client_order_id.clone(),
                    };

                    self.broadcast_event(OrderEvent::OrderPlaced {
                        order: order_info.clone(),
                    });
                    self.broadcast_event(OrderEvent::OrderFilled {
                        order: order_info,
                    });

                    debug!(
                        order_id = %order_id,
                        symbol = %symbol,
                        side = ?side,
                        amount,
                        "Paper market order filled immediately"
                    );
                } else {
                    // Limit order: place into pending, wait for price tick
                    let order = PendingOrder {
                        id: Uuid::new_v4(),
                        symbol,
                        side,
                        amount,
                        price: price.unwrap_or(0.0),
                        reduce_only,
                        client_order_id: client_order_id.clone(),
                        created_at: Utc::now(),
                    };

                    let order_id = order.id;

                    self.pending.lock().await.insert(order_id, order.clone());

                    self.broadcast_event(OrderEvent::OrderPlaced {
                        order: OrderInfo {
                            id: order_id,
                            symbol: order.symbol.clone(),
                            side: order.side,
                            fill_price: None,
                            request_price: Some(order.price),
                            filled: 0.0,
                            client_order_id: order.client_order_id.clone(),
                        },
                    });

                    debug!(
                        order_id = %order_id,
                        symbol = %order.symbol,
                        side = ?order.side,
                        price = order.price,
                        amount = order.amount,
                        "Paper limit order placed"
                    );
                }
            }
            OrderCommand::CancelOrder { order_id, symbol: _ } => {
                let mut pending = self.pending.lock().await;
                if pending.remove(&order_id).is_some() {
                    self.broadcast_event(OrderEvent::OrderCanceled { order_id, symbol: None });
                    debug!(order_id = %order_id, "Paper order canceled");
                } else {
                    debug!(order_id = %order_id, "Paper cancel order not found");
                }
            }
            OrderCommand::CancelAllOrders { symbol: _ } => {
                let mut pending = self.pending.lock().await;
                for order_id in pending.keys() {
                    self.broadcast_event(OrderEvent::OrderCanceled { order_id: *order_id, symbol: None });
                }
                let count = pending.len();
                pending.clear();
                debug!(count, "Paper orders canceled");
            }
            OrderCommand::CloseAllPositions { symbol: _, exchange: _ } => {
                debug!("Paper CloseAllPositions - no-op in paper trading");
            }
        }

        Ok(())
    }
}
