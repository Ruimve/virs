//! Paper Exchange Adapter
//!
//! Paper mode adapter implementing the Position Engine Exchange trait.
//! Market orders fill immediately, Limit orders wait for price trigger.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use dashmap::DashMap;
use tokio::sync::{mpsc, Mutex};
use tokio_stream::wrappers::ReceiverStream;
use tracing::warn;
use uuid::Uuid;

use virs_types::enums::*;
use virs_types::exchange_pe::{ExchangePe, OrderUpdateStream};
use virs_types::market::*;
use virs_types::position::*;

use virs_error::{ExchangeError, VirsError, VirsResult};

use crate::registry::Exchanges;

/// Paper pending order
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

/// Paper position — reuses `ExchangePosition` from `virs-types` so that
/// `unrealized_pnl_at` / `pnl_pct_at` are shared with the rest of the codebase
/// instead of being duplicated here.
type PaperPosition = ExchangePosition;

/// Paper Exchange Adapter
pub struct PaperExchangeAdapter {
    name: String,
    market_type: MarketType,
    pending: Arc<DashMap<Uuid, PaperPendingOrder>>,
    positions: Arc<DashMap<String, PaperPosition>>,
    balance: Arc<Mutex<Balance>>,
    price_tx: Arc<Mutex<Option<mpsc::Sender<WsFeedEvent>>>>,
    last_prices: Arc<DashMap<String, f64>>,
    exchange_registry: Option<Arc<Exchanges>>,
    balance_initialized: Arc<AtomicBool>,
    /// 每个 symbol 配置的杠杆（由 set_leverage 设置，供 update_position_on_fill 使用）
    configured_leverage: Arc<DashMap<String, u32>>,
}

/// Paper 模式简化强平价计算（忽略维持保证金率）：
/// - 多头：entry_price * (1 - 1/leverage)
/// - 空头：entry_price * (1 + 1/leverage)
pub fn compute_paper_liquidation_price(
    entry_price: f64,
    side: PositionSide,
    leverage: u32,
) -> Option<f64> {
    if leverage == 0 || entry_price <= 0.0 {
        return None;
    }
    let ratio = 1.0 / leverage as f64;
    Some(match side {
        PositionSide::Long => entry_price * (1.0 - ratio),
        PositionSide::Short => entry_price * (1.0 + ratio),
    })
}

impl PaperExchangeAdapter {
    pub fn new(name: &str, market_type: MarketType, initial_balance: f64) -> Self {
        Self {
            name: name.to_string(),
            market_type,
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
            exchange_registry: None,
            balance_initialized: Arc::new(AtomicBool::new(initial_balance > 0.0)),
            configured_leverage: Arc::new(DashMap::new()),
        }
    }

    pub fn with_exchange_registry(mut self, registry: Arc<Exchanges>) -> Self {
        self.exchange_registry = Some(registry);
        self
    }

    async fn ensure_balance_initialized(&self) {
        if self.balance_initialized.load(Ordering::Relaxed) {
            return;
        }
        let registry = match &self.exchange_registry {
            Some(r) => r.clone(),
            None => return,
        };
        let exchange = registry
            .registered_names()
            .iter()
            .find(|n| n.contains("perpetual"))
            .and_then(|key| registry.get(key));
        if let Some(ex) = exchange {
            match ex.get_balances().await {
                Ok(balances) => {
                    if let Some(usdt) = balances
                        .iter()
                        .find(|b| b.asset.eq_ignore_ascii_case("USDT"))
                    {
                        let mut balance = self.balance.lock().await;
                        if !self.balance_initialized.load(Ordering::Relaxed) {
                            balance.free = usdt.free;
                            balance.used = usdt.used;
                            balance.total = usdt.total;
                            self.balance_initialized.store(true, Ordering::Relaxed);
                        }
                    }
                }
                Err(e) => {
                    warn!(error = %e, "PaperExchangeAdapter: failed to fetch balance from real exchange");
                }
            }
        }
    }

    pub async fn on_price_tick(&self, symbol: &str, current_price: f64) {
        if current_price <= 0.0 {
            return;
        }
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
            self.update_position_on_fill(order, current_price).await;
            // Paper 模式按 maker 费率计算手续费（限价单）
            let fee = current_price * order.amount * 0.0002;
            let tx = self.price_tx.lock().await;
            if let Some(ref tx) = *tx {
                if tx
                    .send(WsFeedEvent::OrderUpdate {
                        exchange_order_id: order.id.to_string(),
                        symbol: order.symbol.clone(),
                        status: OrderStatus::Filled,
                        filled: order.amount,
                        remaining: 0.0,
                        price: current_price,
                        amount: order.amount,
                        commission: fee,
                        timestamp: Utc::now(),
                        position_side: order.position_side,
                    })
                    .await
                    .is_err()
                {
                    warn!(order_id = %order.id, symbol = %order.symbol, "Paper WsFeedEvent::OrderUpdate send failed — receiver dropped, event lost");
                }
            }
        }
        self.update_unrealized_pnl(symbol, current_price).await;
    }

    async fn update_position_on_fill(&self, order: &PaperPendingOrder, fill_price: f64) {
        // Hedge mode requires position_side — None is an irrecoverable caller bug.
        let position_side = match order.position_side {
            Some(ps) => ps,
            None => {
                tracing::error!(
                    symbol = %order.symbol,
                    order_id = %order.id,
                    "position_side is None in Hedge mode — skipping fill update to avoid \
                     silent position corruption. Caller must provide position_side."
                );
                return;
            }
        };
        let key = format!("{}:{:?}", order.symbol, position_side);
        // Hedge mode: size is always an absolute value. Direction is carried
        // by position_side (the key), not by the sign of size.
        let size_delta = order.amount;
        // Leverage must be explicitly configured via set_leverage.
        // Default to 1 (no leverage) — never 20, as high leverage can cause
        // irreversible liquidation if the caller forgets to set it.
        let leverage: u32 = self
            .configured_leverage
            .get(&order.symbol)
            .map(|v| *v)
            .unwrap_or_else(|| {
                tracing::error!(
                    symbol = %order.symbol,
                    "No leverage configured for symbol — defaulting to 1 (no leverage). \
                     Call set_leverage() before trading to avoid unexpected margin calculations."
                );
                1
            });
        let leverage_f64 = leverage as f64;
        let notional = fill_price * order.amount;
        let margin = notional / leverage_f64;

        let is_opening = match (order.side, position_side) {
            (Side::Buy, PositionSide::Long) => true,
            (Side::Sell, PositionSide::Short) => true,
            (Side::Sell, PositionSide::Long) => false,
            (Side::Buy, PositionSide::Short) => false,
        };

        // ── 1. 保存旧仓位状态（用于计算 realized_pnl，必须在更新前读取） ──
        let old_pos_info = self
            .positions
            .get(&key)
            .map(|p| (p.side, p.entry_price, p.size));

        // ── 2. 计算 realized_pnl（仅平仓时有已实现盈亏） ──
        // Hedge 模式下 Long/Short 分键，不会出现同一 key 的反转，
        // 超量平仓钳制到 0 而非反转 side。
        let realized_pnl: f64 = match (&old_pos_info, is_opening) {
            (Some((side, entry, old_size)), false) => {
                let closed = order.amount.min(*old_size);
                match side {
                    PositionSide::Long => (fill_price - entry) * closed,
                    PositionSide::Short => (entry - fill_price) * closed,
                }
            }
            _ => 0.0,
        };

        // ── 3. 更新仓位 ──
        match self.positions.get_mut(&key) {
            Some(mut pos) => {
                if is_opening {
                    // 加仓：加权平均入场价
                    let old_size = pos.size;
                    let new_size = old_size + size_delta;
                    let total_cost = pos.entry_price * old_size + fill_price * size_delta;
                    pos.size = new_size;
                    pos.entry_price = total_cost / new_size;
                    pos.leverage = leverage;
                    pos.liquidation_price =
                        compute_paper_liquidation_price(pos.entry_price, pos.side, leverage);
                } else {
                    // 平仓：减少 size
                    let new_size = pos.size - size_delta;
                    if new_size < 1e-8 {
                        // 全部平仓（含超量平仓钳制到 0）
                        drop(pos);
                        self.positions.remove(&key);
                    } else {
                        // 部分平仓：入场价不变
                        pos.size = new_size;
                        pos.leverage = leverage;
                        pos.liquidation_price =
                            compute_paper_liquidation_price(pos.entry_price, pos.side, leverage);
                    }
                }
            }
            None => {
                // 新建仓位：side 直接取 position_side
                let liq_price = compute_paper_liquidation_price(fill_price, position_side, leverage);
                self.positions.insert(
                    key.clone(),
                    PaperPosition {
                        symbol: order.symbol.clone(),
                        side: position_side,
                        size: size_delta,
                        entry_price: fill_price,
                        leverage,
                        unrealized_pnl: 0.0,
                        liquidation_price: liq_price,
                    },
                );
            }
        }

        // ── 4. 更新余额 ──
        let mut balance = self.balance.lock().await;

        if is_opening {
            balance.used += margin;
            balance.free -= margin;
        } else {
            // 平仓：释放保证金 + 已实现盈亏
            let margin_release = margin.min(balance.used);
            balance.used -= margin_release;
            balance.free += margin_release + realized_pnl;
        }
    }

    async fn update_unrealized_pnl(&self, symbol: &str, current_price: f64) {
        for mut entry in self.positions.iter_mut() {
            let pos = entry.value_mut();
            if pos.symbol != symbol {
                continue;
            }
            pos.unrealized_pnl = pos.unrealized_pnl_at(current_price);
        }
    }
}

#[async_trait]
impl ExchangePe for PaperExchangeAdapter {
    fn name(&self) -> &str {
        &self.name
    }
    fn market_type(&self) -> MarketType {
        self.market_type
    }

    async fn get_ticker(&self, symbol: &str) -> VirsResult<Ticker> {
        warn!(
            exchange = %self.name,
            symbol = %symbol,
            "PaperExchange get_ticker stub — returning all-zero Ticker (paper mode, no real market data)"
        );
        Ok(Ticker {
            symbol: symbol.to_string(),
            exchange: self.name.clone(),
            bid: 0.0,
            ask: 0.0,
            last: 0.0,
            high_24h: 0.0,
            low_24h: 0.0,
            volume_24h: 0.0,
            price_change_24h: 0.0,
            price_change_pct_24h: 0.0,
            timestamp: Utc::now(),
        })
    }

    async fn get_balance(&self) -> VirsResult<Balance> {
        self.ensure_balance_initialized().await;
        let mut balance = self.balance.lock().await;
        balance.total = balance.compute_total();
        Ok(balance.clone())
    }

    async fn get_positions(&self, symbol: Option<&str>) -> VirsResult<Vec<ExchangePosition>> {
        Ok(self
            .positions
            .iter()
            .filter(|e| {
                let pos = e.value();
                symbol.is_none_or(|s| pos.symbol == s) && pos.size.abs() > 1e-8
            })
            .map(|e| {
                let pos = e.value();
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
            .collect())
    }

    async fn get_funding_rate(&self, symbol: &str) -> VirsResult<FundingRate> {
        warn!(
            exchange = %self.name,
            symbol = %symbol,
            "PaperExchange get_funding_rate stub — returning zero rate (paper mode, no real funding data)"
        );
        Ok(FundingRate {
            symbol: symbol.to_string(),
            rate: 0.0,
            next_funding_time: Some(Utc::now()),
        })
    }

    async fn place_order(&self, params: PlaceOrderParams) -> VirsResult<PositionOrder> {
        let order_id = Uuid::new_v4();
        let now = Utc::now();
        let is_market = params.order_type == OrderType::Market || params.price.is_none();

        if is_market {
            let fill_price = self
                .last_prices
                .get(&params.symbol)
                .map(|r| *r)
                .ok_or_else(|| {
                    tracing::error!(
                        symbol = %params.symbol,
                        "No last price available for paper market order — returning NoData instead of 0.0"
                    );
                    VirsError::Exchange(ExchangeError::no_data(format!(
                        "No last price for paper market order on {}",
                        params.symbol
                    )))
                })?;
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
            self.update_position_on_fill(&pending_for_fill, fill_price)
                .await;

            // Paper 模式按 taker 费率计算手续费（计价货币 USDT）
            let fee = fill_price * params.amount * 0.0005;

            let order = PositionOrder {
                id: order_id,
                position_id: params.position_id.unwrap_or(Uuid::nil()),
                exchange_order_id: Some(order_id.to_string()),
                client_order_id: params.client_order_id.clone(),
                exchange: self.name.clone(),
                symbol: params.symbol.clone(),
                side: params.side,
                order_type: params.order_type,
                request_price: params.price,
                fill_price: if fill_price > 0.0 {
                    Some(fill_price)
                } else {
                    None
                },
                amount: params.amount,
                filled: params.amount,
                remaining: 0.0,
                status: OrderStatus::Filled,
                reduce_only: params.reduce_only,
                fee,
                fee_currency: "USDT".to_string(),
                slippage: None,
                created_at: now,
                updated_at: now,
            };

            let tx = self.price_tx.lock().await;
            if let Some(ref tx) = *tx {
                if tx
                    .send(WsFeedEvent::OrderUpdate {
                        exchange_order_id: order_id.to_string(),
                        symbol: params.symbol.clone(),
                        status: OrderStatus::Filled,
                        filled: params.amount,
                        remaining: 0.0,
                        price: fill_price,
                        amount: params.amount,
                        commission: fee,
                        timestamp: Utc::now(),
                        position_side: params.position_side,
                    })
                    .await
                    .is_err()
                {
                    warn!(order_id = %order_id, symbol = %params.symbol, "Paper WsFeedEvent::OrderUpdate send failed — receiver dropped, event lost");
                }
            }
            Ok(order)
        } else {
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
            let order = PositionOrder {
                id: order_id,
                position_id: params.position_id.unwrap_or(Uuid::nil()),
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
            Ok(order)
        }
    }

    async fn cancel_order(&self, _symbol: &str, order_id: &str) -> VirsResult<PositionOrder> {
        let uuid = Uuid::parse_str(order_id).map_err(|_| {
            ExchangeError::Internal(format!(
                "Invalid order ID: {}",
                order_id
            ))
        })?;
        let now = Utc::now();
        match self.pending.remove(&uuid) {
            Some((_, pending)) => Ok(PositionOrder {
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
            }),
            None => Err(VirsError::Exchange(ExchangeError::OrderNotFound(
                order_id.to_string(),
            ))),
        }
    }

    async fn cancel_all_orders(&self, symbol: Option<&str>) -> VirsResult<Vec<PositionOrder>> {
        let now = Utc::now();
        let keys: Vec<Uuid> = self
            .pending
            .iter()
            .filter(|e| symbol.is_none_or(|s| e.value().symbol == s))
            .map(|e| *e.key())
            .collect();
        let mut canceled = Vec::new();
        for key in keys {
            if let Some((_, pending)) = self.pending.remove(&key) {
                canceled.push(PositionOrder {
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
        Ok(canceled)
    }

    async fn get_open_orders(&self, symbol: Option<&str>) -> VirsResult<Vec<PositionOrder>> {
        Ok(self
            .pending
            .iter()
            .filter(|e| symbol.is_none_or(|s| e.value().symbol == s))
            .map(|e| {
                let o = e.value();
                PositionOrder {
                    id: o.id,
                    position_id: Uuid::nil(),
                    exchange_order_id: Some(o.id.to_string()),
                    client_order_id: o.client_order_id.clone(),
                    exchange: self.name.clone(),
                    symbol: o.symbol.clone(),
                    side: o.side,
                    order_type: o.order_type,
                    request_price: o.price,
                    fill_price: None,
                    amount: o.amount,
                    filled: 0.0,
                    remaining: o.amount,
                    status: OrderStatus::Open,
                    reduce_only: o.reduce_only,
                    fee: 0.0,
                    fee_currency: "USDT".to_string(),
                    slippage: None,
                    created_at: o.created_at,
                    updated_at: o.created_at,
                }
            })
            .collect())
    }

    async fn get_order(&self, _symbol: &str, order_id: &str) -> VirsResult<PositionOrder> {
        let uuid = Uuid::parse_str(order_id).map_err(|_| {
            VirsError::Exchange(ExchangeError::Internal(format!(
                "Invalid order ID: {}",
                order_id
            )))
        })?;
        match self.pending.get(&uuid) {
            Some(o) => Ok(PositionOrder {
                id: o.id,
                position_id: Uuid::nil(),
                exchange_order_id: Some(o.id.to_string()),
                client_order_id: o.client_order_id.clone(),
                exchange: self.name.clone(),
                symbol: o.symbol.clone(),
                side: o.side,
                order_type: o.order_type,
                request_price: o.price,
                fill_price: None,
                amount: o.amount,
                filled: 0.0,
                remaining: o.amount,
                status: OrderStatus::Open,
                reduce_only: o.reduce_only,
                fee: 0.0,
                fee_currency: "USDT".to_string(),
                slippage: None,
                created_at: o.created_at,
                updated_at: o.created_at,
            }),
            None => Err(VirsError::Exchange(ExchangeError::OrderNotFound(
                order_id.to_string(),
            ))),
        }
    }

    async fn set_leverage(&self, symbol: &str, leverage: u32) -> VirsResult<()> {
        // 保存 symbol 对应的 leverage，供 update_position_on_fill 使用
        self.configured_leverage
            .insert(symbol.to_string(), leverage);
        Ok(())
    }

    async fn get_position_mode(&self) -> VirsResult<PositionMode> {
        // Proxy to the real perpetual exchange when available — paper mode
        // should reflect the actual account's position mode, not a hard-coded
        // value. If no registry is attached (pure paper without a real
        // exchange), default to Hedge.
        let registry = match &self.exchange_registry {
            Some(r) => r.clone(),
            None => return Ok(PositionMode::Hedge),
        };
        let exchange = registry
            .registered_names()
            .iter()
            .find(|n| n.contains("perpetual"))
            .and_then(|key| registry.get(key));
        match exchange {
            Some(ex) => ex.get_position_mode().await.map_err(Into::into),
            None => Ok(PositionMode::Hedge),
        }
    }

    async fn subscribe_order_updates(
        &self,
        _symbols: &[&str],
    ) -> VirsResult<OrderUpdateStream> {
        let (tx, rx) = mpsc::channel(256);
        let mut price_tx = self.price_tx.lock().await;
        *price_tx = Some(tx);
        Ok(Box::pin(ReceiverStream::new(rx)))
    }

    async fn on_price_tick(&self, symbol: &str, price: f64) {
        PaperExchangeAdapter::on_price_tick(self, symbol, price).await;
    }

    /// Paper 模式从 DB 恢复仓位到内存。
    /// 进程重启后 Paper 的 `positions` 内存丢失，需由 PE 在 recover_state 时调用此方法恢复。
    async fn restore_positions(&self, positions: Vec<ExchangePosition>) {
        for pos in positions {
            // 跳过 size 为 0 的无效仓位
            if pos.size.abs() < 1e-8 {
                continue;
            }
            let key = format!("{}:{:?}", pos.symbol, pos.side);
            self.positions.insert(
                key.clone(),
                PaperPosition {
                    symbol: pos.symbol.clone(),
                    side: pos.side,
                    size: pos.size,
                    entry_price: pos.entry_price,
                    leverage: pos.leverage,
                    unrealized_pnl: pos.unrealized_pnl,
                    liquidation_price: pos.liquidation_price,
                },
            );
            // 同步 entry_price 作为 last_price（避免后续下单 fill_price=0）
            self.last_prices.insert(pos.symbol.clone(), pos.entry_price);
        }
    }
}
