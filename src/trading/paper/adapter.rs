//! Paper Exchange Adapter
//!
//! 将 Paper 模拟交易适配为 Position Engine 的 Exchange trait。
//! Paper 模式下不发送真实订单，Market 单立即成交，Limit 单挂单等待价格触发。

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use dashmap::DashMap;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, info};
use uuid::Uuid;

use crate::engine::position::error::{PositionEngineError, Result};
use crate::engine::position::exchange::Exchange as PeExchange;
use crate::engine::position::types::*;

/// Paper 模拟挂单
#[derive(Debug, Clone)]
struct PaperPendingOrder {
    id: Uuid,
    symbol: String,
    side: Side,
    order_type: OrderType,
    amount: f64,
    price: Option<f64>,
    reduce_only: bool,
    position_side: Option<PositionSide>,
    client_order_id: Option<String>,
    created_at: chrono::DateTime<Utc>,
}

/// Paper 模拟持仓
#[derive(Debug, Clone)]
struct PaperPosition {
    symbol: String,
    side: PositionSide,
    size: f64,
    entry_price: f64,
    leverage: u32,
    unrealized_pnl: f64,
    liquidation_price: Option<f64>,
}

/// Paper Exchange Adapter
///
/// 实现 Position Engine 的 Exchange trait，用于纸面交易。
/// - Market 单立即成交
/// - Limit 单挂单，通过 on_price_tick 触发成交
/// - 模拟余额和持仓
pub struct PaperExchangeAdapter {
    name: String,
    market_type: MarketType,
    position_mode: PositionMode,
    pending: Arc<DashMap<Uuid, PaperPendingOrder>>,
    positions: Arc<DashMap<String, PaperPosition>>,
    /// 模拟余额
    balance: Arc<Mutex<Balance>>,
    /// 价格 tick 通道，用于通知 WS 模拟
    price_tx: Arc<Mutex<Option<mpsc::Sender<WsFeedEvent>>>>,
    /// 每个 symbol 的最新价格（由 on_price_tick 更新）
    last_prices: Arc<DashMap<String, f64>>,
}

impl PaperExchangeAdapter {
    pub fn new(name: &str, market_type: MarketType, initial_balance: f64) -> Self {
        Self::with_position_mode(name, market_type, initial_balance, PositionMode::Hedge)
    }

    pub fn with_position_mode(name: &str, market_type: MarketType, initial_balance: f64, position_mode: PositionMode) -> Self {
        info!(
            name,
            market_type = ?market_type,
            position_mode = ?position_mode,
            initial_balance,
            "PaperExchangeAdapter created"
        );
        Self {
            name: name.to_string(),
            market_type,
            position_mode,
            pending: Arc::new(DashMap::new()),
            positions: Arc::new(DashMap::new()),
            balance: Arc::new(Mutex::new(Balance {
                asset: "USDT".to_string(),
                free: initial_balance,
                used: 0.0,
                total: initial_balance,
            })),
            price_tx: Arc::new(Mutex::new(None)),
            last_prices: Arc::new(DashMap::new()),
        }
    }

    /// 价格 tick — 检查挂单是否触发成交
    ///
    /// 买单：当前价格 <= 挂单价 → 成交
    /// 卖单：当前价格 >= 挂单价 → 成交
    pub async fn on_price_tick(&self, symbol: &str, current_price: f64) {
        if current_price <= 0.0 {
            return;
        }

        // 更新最新价格
        self.last_prices.insert(symbol.to_string(), current_price);

        let mut triggered = Vec::new();

        for entry in self.pending.iter() {
            let order = entry.value();
            if order.symbol != symbol {
                continue;
            }

            let filled = match order.side {
                Side::Buy => current_price <= order.price.unwrap_or(current_price),
                Side::Sell => current_price >= order.price.unwrap_or(current_price),
            };

            if filled {
                triggered.push(order.clone());
            }
        }

        for order in &triggered {
            self.pending.remove(&order.id);

            // 更新模拟持仓
            self.update_position_on_fill(order, current_price).await;

            // 通知 WS 模拟通道
            let tx = self.price_tx.lock().await;
            if let Some(ref tx) = *tx {
                let _ = tx.send(WsFeedEvent::OrderUpdate {
                    exchange_order_id: order.id.to_string(),
                    symbol: order.symbol.clone(),
                    status: OrderStatus::Filled,
                    filled: order.amount,
                    remaining: 0.0,
                    price: current_price,
                    amount: order.amount,
                    commission: 0.0,
                    timestamp: Utc::now(),
                    position_side: order.position_side,
                }).await;
            }

            debug!(
                order_id = %order.id,
                symbol = %order.symbol,
                side = ?order.side,
                price = ?order.price,
                fill_price = current_price,
                amount = order.amount,
                "Paper limit order filled via price tick"
            );
        }

        // 更新持仓的未实现盈亏
        self.update_unrealized_pnl(symbol, current_price).await;
    }

    async fn update_position_on_fill(&self, order: &PaperPendingOrder, fill_price: f64) {
        let key = format!("{}:{:?}", order.symbol, order.position_side.unwrap_or(PositionSide::Both));

        let size_delta = if order.side == Side::Buy { order.amount } else { -order.amount };

        match self.positions.get_mut(&key) {
            Some(mut pos) => {
                let old_size = pos.size;
                let new_size = old_size + size_delta;

                if new_size.abs() < 1e-8 {
                    // 平仓
                    drop(pos);
                    self.positions.remove(&key);
                    debug!(symbol = %order.symbol, "Paper position closed");
                } else if old_size * new_size < 0.0 {
                    // 反向
                    pos.size = new_size;
                    pos.entry_price = fill_price;
                    pos.side = if new_size > 0.0 { PositionSide::Long } else { PositionSide::Short };
                    debug!(symbol = %order.symbol, new_size, "Paper position reversed");
                } else {
                    // 加仓/减仓
                    let total_cost = pos.entry_price * old_size.abs() + fill_price * size_delta.abs();
                    pos.size = new_size;
                    if new_size.abs() > 0.0 {
                        pos.entry_price = total_cost / new_size.abs();
                    }
                    debug!(symbol = %order.symbol, new_size, entry_price = pos.entry_price, "Paper position updated");
                }
            }
            None => {
                let side = if size_delta > 0.0 { PositionSide::Long } else { PositionSide::Short };
                let pos = PaperPosition {
                    symbol: order.symbol.clone(),
                    side,
                    size: size_delta,
                    entry_price: fill_price,
                    leverage: 20,
                    unrealized_pnl: 0.0,
                    liquidation_price: None,
                };
                self.positions.insert(key, pos);
                debug!(symbol = %order.symbol, side = ?side, size = size_delta, "Paper position opened");
            }
        }

        // 更新余额
        let mut balance = self.balance.lock().await;
        let cost = fill_price * order.amount;
        if order.side == Side::Buy {
            balance.used += cost;
            balance.free -= cost;
        } else {
            balance.used -= cost.min(balance.used);
            balance.free += cost;
        }
    }

    async fn update_unrealized_pnl(&self, symbol: &str, current_price: f64) {
        for mut entry in self.positions.iter_mut() {
            let pos = entry.value_mut();
            if pos.symbol != symbol {
                continue;
            }
            pos.unrealized_pnl = match pos.side {
                PositionSide::Long => (current_price - pos.entry_price) * pos.size,
                PositionSide::Short => (pos.entry_price - current_price) * pos.size.abs(),
                PositionSide::Both => 0.0,
            };
        }
    }
}

#[async_trait]
impl PeExchange for PaperExchangeAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    fn market_type(&self) -> MarketType {
        self.market_type
    }

    // ── 行情数据 ──

    async fn get_ticker(&self, symbol: &str) -> Result<Ticker> {
        // Paper 模式不提供真实行情，返回占位数据
        Ok(Ticker {
            symbol: symbol.to_string(),
            price: 0.0,
            bid: 0.0,
            ask: 0.0,
            volume_24h: 0.0,
            timestamp: Utc::now(),
        })
    }

    async fn get_balance(&self) -> Result<Balance> {
        let balance = self.balance.lock().await;
        Ok(balance.clone())
    }

    async fn get_positions(&self, symbol: Option<&str>) -> Result<Vec<ExchangePosition>> {
        let positions: Vec<ExchangePosition> = self.positions
            .iter()
            .filter(|entry| {
                let pos = entry.value();
                symbol.map_or(true, |s| pos.symbol == s) && pos.size.abs() > 1e-8
            })
            .map(|entry| {
                let pos = entry.value();
                ExchangePosition {
                    symbol: pos.symbol.clone(),
                    side: pos.side,
                    size: pos.size,
                    entry_price: pos.entry_price,
                    leverage: pos.leverage,
                    unrealized_pnl: pos.unrealized_pnl,
                    liquidation_price: pos.liquidation_price,
                }
            })
            .collect();
        Ok(positions)
    }

    async fn get_funding_rate(&self, symbol: &str) -> Result<FundingRate> {
        Ok(FundingRate {
            symbol: symbol.to_string(),
            rate: 0.0,
            next_funding_time: Utc::now(),
        })
    }

    async fn get_fee_rates(&self, symbol: &str) -> Result<FeeRates> {
        Ok(FeeRates {
            symbol: symbol.to_string(),
            maker_rate: 0.0,
            taker_rate: 0.0,
        })
    }

    // ── 交易 ──

    async fn place_order(&self, params: PlaceOrderParams) -> Result<Order> {
        let order_id = Uuid::new_v4();
        let now = Utc::now();

        let is_market = params.order_type == OrderType::Market || params.price.is_none();

        if is_market {
            // Market 单：立即成交，使用最新价格作为 fill_price
            let fill_price = self.last_prices
                .get(&params.symbol)
                .map(|r| *r)
                .unwrap_or(0.0);

            // 更新模拟持仓
            let pending_for_fill = PaperPendingOrder {
                id: order_id,
                symbol: params.symbol.clone(),
                side: params.side,
                order_type: params.order_type,
                amount: params.amount,
                price: Some(fill_price),
                reduce_only: params.reduce_only,
                position_side: params.position_side,
                client_order_id: params.client_order_id.clone(),
                created_at: now,
            };
            self.update_position_on_fill(&pending_for_fill, fill_price).await;

            let order = Order {
                id: order_id,
                position_id: Uuid::nil(),
                exchange_order_id: Some(order_id.to_string()),
                client_order_id: params.client_order_id.clone(),
                exchange: self.name.clone(),
                symbol: params.symbol.clone(),
                side: params.side,
                order_type: params.order_type,
                request_price: params.price,
                fill_price: if fill_price > 0.0 { Some(fill_price) } else { None },
                amount: params.amount,
                filled: params.amount,
                remaining: 0.0,
                status: OrderStatus::Filled,
                reduce_only: params.reduce_only,
                fee: 0.0,
                fee_currency: "USDT".to_string(),
                slippage: None,
                created_at: now,
                updated_at: now,
            };

            // 通知 WS 模拟通道（让 PositionEngine 的 ws_feed_loop 处理成交）
            let tx = self.price_tx.lock().await;
            if let Some(ref tx) = *tx {
                let _ = tx.send(WsFeedEvent::OrderUpdate {
                    exchange_order_id: order_id.to_string(),
                    symbol: params.symbol.clone(),
                    status: OrderStatus::Filled,
                    filled: params.amount,
                    remaining: 0.0,
                    price: fill_price,
                    amount: params.amount,
                    commission: 0.0,
                    timestamp: Utc::now(),
                    position_side: params.position_side,
                }).await;
            }

            info!(
                order_id = %order_id,
                symbol = %params.symbol,
                side = ?params.side,
                amount = params.amount,
                fill_price,
                "Paper market order filled immediately"
            );

            Ok(order)
        } else {
            // Limit 单：挂单等待价格触发
            let pending = PaperPendingOrder {
                id: order_id,
                symbol: params.symbol.clone(),
                side: params.side,
                order_type: params.order_type,
                amount: params.amount,
                price: params.price,
                reduce_only: params.reduce_only,
                position_side: params.position_side,
                client_order_id: params.client_order_id.clone(),
                created_at: now,
            };

            self.pending.insert(order_id, pending);

            let order = Order {
                id: order_id,
                position_id: Uuid::nil(),
                exchange_order_id: Some(order_id.to_string()),
                client_order_id: params.client_order_id,
                exchange: self.name.clone(),
                symbol: params.symbol.clone(),
                side: params.side,
                order_type: params.order_type,
                request_price: params.price,
                fill_price: None,
                amount: params.amount,
                filled: 0.0,
                remaining: params.amount,
                status: OrderStatus::Open,
                reduce_only: params.reduce_only,
                fee: 0.0,
                fee_currency: "USDT".to_string(),
                slippage: None,
                created_at: now,
                updated_at: now,
            };

            debug!(
                order_id = %order_id,
                symbol = %params.symbol,
                side = ?params.side,
                price = ?params.price,
                amount = params.amount,
                "Paper limit order placed"
            );

            Ok(order)
        }
    }

    async fn cancel_order(&self, _symbol: &str, order_id: &str) -> Result<Order> {
        let uuid = Uuid::parse_str(order_id)
            .map_err(|_| PositionEngineError::Exchange(format!("Invalid order ID: {}", order_id)))?;

        let now = Utc::now();

        match self.pending.remove(&uuid) {
            Some((_, pending)) => {
                let order = Order {
                    id: uuid,
                    position_id: Uuid::nil(),
                    exchange_order_id: Some(uuid.to_string()),
                    client_order_id: pending.client_order_id,
                    exchange: self.name.clone(),
                    symbol: pending.symbol,
                    side: pending.side,
                    order_type: pending.order_type,
                    request_price: pending.price,
                    fill_price: None,
                    amount: pending.amount,
                    filled: 0.0,
                    remaining: pending.amount,
                    status: OrderStatus::Canceled,
                    reduce_only: pending.reduce_only,
                    fee: 0.0,
                    fee_currency: "USDT".to_string(),
                    slippage: None,
                    created_at: pending.created_at,
                    updated_at: now,
                };
                debug!(order_id = %uuid, "Paper order canceled");
                Ok(order)
            }
            None => Err(PositionEngineError::Exchange(format!("Order not found: {}", order_id))),
        }
    }

    async fn cancel_all_orders(&self, symbol: Option<&str>) -> Result<Vec<Order>> {
        let now = Utc::now();
        let mut canceled = Vec::new();

        let keys: Vec<Uuid> = self.pending
            .iter()
            .filter(|entry| {
                let order = entry.value();
                symbol.map_or(true, |s| order.symbol == s)
            })
            .map(|entry| *entry.key())
            .collect();

        for key in keys {
            if let Some((_, pending)) = self.pending.remove(&key) {
                canceled.push(Order {
                    id: key,
                    position_id: Uuid::nil(),
                    exchange_order_id: Some(key.to_string()),
                    client_order_id: pending.client_order_id,
                    exchange: self.name.clone(),
                    symbol: pending.symbol,
                    side: pending.side,
                    order_type: pending.order_type,
                    request_price: pending.price,
                    fill_price: None,
                    amount: pending.amount,
                    filled: 0.0,
                    remaining: pending.amount,
                    status: OrderStatus::Canceled,
                    reduce_only: pending.reduce_only,
                    fee: 0.0,
                    fee_currency: "USDT".to_string(),
                    slippage: None,
                    created_at: pending.created_at,
                    updated_at: now,
                });
            }
        }

        debug!(count = canceled.len(), "Paper orders canceled");
        Ok(canceled)
    }

    async fn get_open_orders(&self, symbol: Option<&str>) -> Result<Vec<Order>> {
        let orders: Vec<Order> = self.pending
            .iter()
            .filter(|entry| {
                let order = entry.value();
                symbol.map_or(true, |s| order.symbol == s)
            })
            .map(|entry| {
                let order = entry.value();
                Order {
                    id: order.id,
                    position_id: Uuid::nil(),
                    exchange_order_id: Some(order.id.to_string()),
                    client_order_id: order.client_order_id.clone(),
                    exchange: self.name.clone(),
                    symbol: order.symbol.clone(),
                    side: order.side,
                    order_type: order.order_type,
                    request_price: order.price,
                    fill_price: None,
                    amount: order.amount,
                    filled: 0.0,
                    remaining: order.amount,
                    status: OrderStatus::Open,
                    reduce_only: order.reduce_only,
                    fee: 0.0,
                    fee_currency: "USDT".to_string(),
                    slippage: None,
                    created_at: order.created_at,
                    updated_at: order.created_at,
                }
            })
            .collect();
        Ok(orders)
    }

    async fn get_order(&self, _symbol: &str, order_id: &str) -> Result<Order> {
        let uuid = Uuid::parse_str(order_id)
            .map_err(|_| PositionEngineError::Exchange(format!("Invalid order ID: {}", order_id)))?;

        match self.pending.get(&uuid) {
            Some(order) => Ok(Order {
                id: order.id,
                position_id: Uuid::nil(),
                exchange_order_id: Some(order.id.to_string()),
                client_order_id: order.client_order_id.clone(),
                exchange: self.name.clone(),
                symbol: order.symbol.clone(),
                side: order.side,
                order_type: order.order_type,
                request_price: order.price,
                fill_price: None,
                amount: order.amount,
                filled: 0.0,
                remaining: order.amount,
                status: OrderStatus::Open,
                reduce_only: order.reduce_only,
                fee: 0.0,
                fee_currency: "USDT".to_string(),
                slippage: None,
                created_at: order.created_at,
                updated_at: order.created_at,
            }),
            None => Err(PositionEngineError::Exchange(format!("Order not found: {}", order_id))),
        }
    }

    // ── 永续合约 ──

    async fn set_leverage(&self, _symbol: &str, _leverage: u32) -> Result<()> {
        // Paper 模式下无需设置杠杆
        Ok(())
    }

    async fn get_position_mode(&self) -> Result<PositionMode> {
        Ok(self.position_mode)
    }

    // ── WebSocket 成交回报 ──

    async fn subscribe_order_updates(&self, _symbols: &[&str]) -> Result<mpsc::Receiver<WsFeedEvent>> {
        let (tx, rx) = mpsc::channel(256);
        let mut guard = self.price_tx.lock().await;
        *guard = Some(tx);
        info!("PaperExchangeAdapter: subscribed to order updates (simulated via on_price_tick)");
        Ok(rx)
    }
}
