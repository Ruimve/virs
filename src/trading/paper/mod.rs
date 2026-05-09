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
    created_at: chrono::DateTime<Utc>,
}

/// Paper 交易执行器
///
/// 实现 `OrderExecutor` trait，本地维护挂单簿。
/// 不发送真实订单，而是等待 `on_price_tick` 检查触发条件后
/// 通过 `event_tx` 发送模拟的 `OrderEvent`。
pub struct PaperOrderExecutor {
    /// 挂单簿: order_id -> PendingOrder
    pending: Arc<Mutex<HashMap<Uuid, PendingOrder>>>,
    /// 事件发送通道（将模拟事件发送给 GridWorker）
    event_tx: broadcast::Sender<OrderEvent>,
    /// 是否启用
    enabled: Arc<std::sync::atomic::AtomicBool>,
}

impl PaperOrderExecutor {
    pub fn new(event_tx: broadcast::Sender<OrderEvent>) -> Self {
        Self {
            pending: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
            enabled: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        }
    }

    /// 是否启用 paper 交易
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
            let _ = self.event_tx.send(OrderEvent::OrderCanceled { order_id: *order_id });
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
            side: order.side,
            fill_price: Some(fill_price),
            request_price: Some(order.price),
            filled: order.amount,
        };

        // 先发送 OrderPlaced（如果 worker 之前没收到的话）
        let _ = self.event_tx.send(OrderEvent::OrderPlaced {
            order: order_info.clone(),
        });

        // 再发送 OrderFilled
        let _ = self.event_tx.send(OrderEvent::OrderFilled {
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
            } => {
                let order = PendingOrder {
                    id: Uuid::new_v4(),
                    symbol,
                    side,
                    amount,
                    price: price.unwrap_or(0.0),
                    reduce_only,
                    created_at: Utc::now(),
                };

                let order_id = order.id;

                // 记录挂单
                self.pending.lock().await.insert(order_id, order.clone());

                // 通知 worker 订单已挂出
                let _ = self.event_tx.send(OrderEvent::OrderPlaced {
                    order: OrderInfo {
                        id: order_id,
                        side: order.side,
                        fill_price: None,
                        request_price: Some(order.price),
                        filled: 0.0,
                    },
                });

                debug!(
                    order_id = %order_id,
                    symbol = %order.symbol,
                    side = ?order.side,
                    price = order.price,
                    amount = order.amount,
                    "Paper order placed"
                );
            }
            OrderCommand::CancelAllOrders { symbol: _ } => {
                let mut pending = self.pending.lock().await;
                for order_id in pending.keys() {
                    let _ = self.event_tx.send(OrderEvent::OrderCanceled { order_id: *order_id });
                }
                let count = pending.len();
                pending.clear();
                debug!(count, "Paper orders canceled");
            }
        }

        Ok(())
    }
}
