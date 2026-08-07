use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use tokio::sync::{mpsc, Mutex};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{error, warn};

use virs_type::*;
use virs_type::{
    CcxtOrder, CcxtOrderStatus, ExecutionType, ExchangePe, OrderResult, OrderUpdateStream,
};

use virs_error::{ExchangeError, VirsError, VirsResult};

#[derive(Debug, Clone)]
struct PaperPendingOrder {
    id: i64,
    symbol: String,
    side: Side,
    order_type: OrderType,
    amount: f64,
    price: Option<f64>,
    position_side: Option<PositionSide>,
    client_order_id: String,
}

type PaperPosition = ExchangePosition;


/* 模拟交易适配器：维护本地持仓、余额和挂单，通过价格回调撮合限价单，不连接真实交易所 */
pub struct PaperExchangeAdapter {
    name: String,
    market_type: MarketType,
    /* 挂单池：限价单等待价格触发后撮合 */
    pending: Arc<DashMap<i64, PaperPendingOrder>>,
    /* 持仓表：key 格式为 "symbol:PositionSide"，支持双向持仓 */
    positions: Arc<DashMap<String, PaperPosition>>,
    balance: Arc<Mutex<Balance>>,
    /* 订单更新事件通道：撮合后将 WsFeedEvent::OrderUpdate 推送给订阅者 */
    price_tx: Arc<Mutex<Option<mpsc::Sender<WsFeedEvent>>>>,
    /* 最新价格缓存：市价单撮合和限价单触发都依赖此价格 */
    last_prices: Arc<DashMap<String, f64>>,

    configured_leverage: Arc<DashMap<String, u32>>,
    order_id_counter: Arc<AtomicI64>,
    trade_id_counter: Arc<AtomicI64>,
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
            configured_leverage: Arc::new(DashMap::new()),
            order_id_counter: Arc::new(AtomicI64::new(1)),
            trade_id_counter: Arc::new(AtomicI64::new(1)),
        }
    }


    fn next_order_id(&self) -> i64 {
        self.order_id_counter
            .fetch_add(1, Ordering::Relaxed)
    }


    fn next_trade_id(&self) -> i64 {
        self.trade_id_counter
            .fetch_add(1, Ordering::Relaxed)
    }

    /* 价格回调：收到新价格时检查所有限价单是否触发，触发后撮合并推送订单更新事件 */
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
            /* 限价单撮合规则：买单当价格 <= 限价时成交，卖单当价格 >= 限价时成交 */
            let filled = match &order.side {
                Side::Buy => match order.price {
                    Some(price) => current_price <= price,
                    None => {
                        error!(order_id = %order.id, symbol = %order.symbol, "Buy order has no limit price — skipping");
                        continue;
                    }
                },
                Side::Sell => match order.price {
                    Some(price) => current_price >= price,
                    None => {
                        error!(order_id = %order.id, symbol = %order.symbol, "Sell order has no limit price — skipping");
                        continue;
                    }
                },
                Side::Unknown(_) => unreachable!("validate ensures side is Buy/Sell"),
            };
            if filled {
                triggered.push(order.clone());
            }
        }

        for order in &triggered {
            self.pending.remove(&order.id);
            let realized_pnl = match self.update_position_on_fill(order, current_price).await {
                Ok(pnl) => pnl,
                Err(e) => {
                    error!(
                        symbol = %order.symbol,
                        order_id = %order.id,
                        error = %e,
                        "Failed to update position on fill — skipping order"
                    );
                    continue;
                }
            };

            /* 限价单手续费率 0.02%（maker 费率） */
            let fee = current_price * order.amount * 0.0002;
            let ccxt_order = CcxtOrder {
                order_id: order.id,
                client_order_id: order.client_order_id.clone(),
                symbol: order.symbol.clone(),
                side: order.side.clone(),
                order_type: order.order_type.clone(),
                position_side: order
                    .position_side
                    .clone()
                    .expect("engine always sends Some(Long/Short) to paper exchange"),
                original_order_type: order.order_type.clone(),
                status: CcxtOrderStatus::Filled,
                execution_type: ExecutionType::Trade,
                orig_qty: order.amount.to_string(),
                original_price: order.price.map(|p| p.to_string()).unwrap_or_else(|| "0".to_string()),
                avg_fill_price: current_price.to_string(),
                filled_qty: order.amount.to_string(),
                last_fill_qty: order.amount.to_string(),
                last_fill_price: current_price.to_string(),
                stop_price: "0".to_string(),
                commission: fee.to_string(),
                commission_asset: "USDT".to_string(),
                realized_pnl: realized_pnl.to_string(),
                reduce_only: false,
                is_maker: true,
                close_position: None,
                time_in_force: "GTC".to_string(),
                working_type: "CONTRACT_PRICE".to_string(),
                bids_notional: "0".to_string(),
                ask_notional: "0".to_string(),
                activation_price: None,
                callback_rate: None,
                price_protection: false,
                stp_mode: "NONE".to_string(),
                price_match_mode: "NONE".to_string(),
                gtd_auto_cancel_time: 0,
                expiry_reason: "0".to_string(),
                si: Some(0),
                ss: Some(0),
                trade_time: chrono::Utc::now().timestamp_millis(),
                trade_id: self.next_trade_id(),
                modify_id: None,
                envelope_event_type: "ORDER_TRADE_UPDATE".to_string(),
                envelope_event_time: chrono::Utc::now().timestamp_millis(),
                envelope_transaction_time: chrono::Utc::now().timestamp_millis(),
            };
            let tx = self.price_tx.lock().await;
            if let Some(ref tx) = *tx {
                if tx
                    .send(WsFeedEvent::OrderUpdate { order: Arc::new(ccxt_order) })
                    .await
                    .is_err()
                {
                    warn!(order_id = %order.id, symbol = %order.symbol, "Paper WsFeedEvent::OrderUpdate send failed — receiver dropped, event lost");
                }
            }
        }
    }

    /* 订单成交后更新持仓和余额：计算已实现盈亏、更新持仓均价和数量、调整保证金占用 */
    async fn update_position_on_fill(
        &self,
        order: &PaperPendingOrder,
        fill_price: f64,
    ) -> VirsResult<f64> {
        let position_side = match &order.position_side {
            Some(ps) => ps.clone(),
            None => {
                return Err(VirsError::config(format!(
                    "position_side is None for order {} on {} — caller must provide position_side in Hedge mode",
                    order.id, order.symbol
                )));
            }
        };
        let key = format!("{}:{:?}", order.symbol, position_side);

        let size_delta = order.amount;

        let leverage: u32 = self
            .configured_leverage
            .get(&order.symbol)
            .map(|v| *v)
            .ok_or_else(|| {
                VirsError::config(format!(
                    "No leverage configured for {} — call set_leverage() before trading",
                    order.symbol
                ))
            })?;
        let leverage_f64 = leverage as f64;
        let notional = fill_price * order.amount;
        /* 保证金 = 名义价值 / 杠杆倍数 */
        let margin = notional / leverage_f64;

        /* 判断是开仓还是平仓：买多/卖空为开仓，卖多/买空为平仓 */
        let is_opening = match (&order.side, &position_side) {
            (Side::Buy, PositionSide::Long) => true,
            (Side::Sell, PositionSide::Short) => true,
            (Side::Sell, PositionSide::Long) => false,
            (Side::Buy, PositionSide::Short) => false,
            (Side::Unknown(_), _) | (_, PositionSide::Unknown(_)) => {
                unreachable!("validate ensures side/position_side are known")
            }
        };

        let old_pos_info = self
            .positions
            .get(&key)
            .map(|p| (p.side.clone(), p.entry_price, p.quantity));

        /* 平仓时计算已实现盈亏：多头 (成交价 - 开仓价) * 平仓数量，空头反之 */
        let realized_pnl: f64 = match (&old_pos_info, is_opening) {
            (Some((side, entry, old_qty)), false) => {
                let closed = order.amount.min(*old_qty);
                match side {
                    PositionSide::Long => (fill_price - entry) * closed,
                    PositionSide::Short => (entry - fill_price) * closed,
                    PositionSide::Unknown(_) => {
                        unreachable!("validate ensures position_side is Long/Short")
                    }
                }
            }
            _ => 0.0,
        };

        match self.positions.get_mut(&key) {
            Some(mut pos) => {
                if is_opening {
                    /* 开仓：新持仓均价 = (旧均价 * 旧数量 + 成交价 * 新增数量) / 总数量 */
                    let old_qty = pos.quantity;
                    let new_qty = old_qty + size_delta;
                    let total_cost = pos.entry_price * old_qty + fill_price * size_delta;
                    pos.quantity = new_qty;
                    pos.entry_price = total_cost / new_qty;
                } else {
                    /* 平仓：减少持仓数量，数量低于阈值时清除持仓 */
                    let new_qty = pos.quantity - size_delta;
                    if new_qty < 1e-8 {
                        drop(pos);
                        self.positions.remove(&key);
                    } else {
                        pos.quantity = new_qty;
                    }
                }
            }
            None => {
                self.positions.insert(
                    key.clone(),
                    PaperPosition {
                        symbol: order.symbol.clone(),
                        side: position_side,
                        quantity: size_delta,
                        entry_price: fill_price,
                        margin_mode: MarginMode::Cross,
                        info: serde_json::Value::Null,
                    },
                );
            }
        }

        let mut balance = self.balance.lock().await;

        /* 开仓冻结保证金，平仓释放保证金并计入已实现盈亏 */
        if is_opening {
            balance.used += margin;
            balance.free -= margin;
        } else {
            let margin_release = margin.min(balance.used);
            balance.used -= margin_release;
            balance.free += margin_release + realized_pnl;
        }

        Ok(realized_pnl)
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
        Err(VirsError::Exchange(ExchangeError::NotSupported(
            format!(
                "PaperExchange does not support get_ticker for {} — use PaperModeExchange for routing",
                symbol
            )
        )))
    }

    async fn get_klines(
        &self,
        _symbol: &str,
        _interval: &str,
        _limit: u32,
        _since: Option<i64>,
    ) -> VirsResult<Vec<Kline>> {
        Err(VirsError::Exchange(ExchangeError::NotSupported(
            "PaperExchange does not support get_klines — use PaperModeExchange for routing".into(),
        )))
    }

    async fn get_klines_range(
        &self,
        _symbol: &str,
        _interval: &str,
        _start_ms: i64,
        _end_ms: i64,
    ) -> VirsResult<Vec<Kline>> {
        Err(VirsError::Exchange(ExchangeError::NotSupported(
            "PaperExchange does not support get_klines_range — use PaperModeExchange for routing".into(),
        )))
    }

    async fn get_funding_rate(&self, symbol: &str) -> VirsResult<FundingRate> {
        Err(VirsError::Exchange(ExchangeError::NotSupported(
            format!(
                "PaperExchange does not support get_funding_rate for {} — use PaperModeExchange for routing",
                symbol
            )
        )))
    }

    async fn get_symbols(&self) -> VirsResult<Vec<String>> {
        Err(VirsError::Exchange(ExchangeError::NotSupported(
            "PaperExchange does not support get_symbols — use PaperModeExchange for routing".into(),
        )))
    }

    async fn get_min_qty(&self, _symbol: &str) -> VirsResult<f64> {
        Err(VirsError::Exchange(ExchangeError::NotSupported(
            "PaperExchange does not support get_min_qty — use PaperModeExchange for routing".into(),
        )))
    }

    async fn create_listen_key(&self) -> VirsResult<String> {
        Err(VirsError::Exchange(ExchangeError::NotSupported(
            "PaperExchange does not support create_listen_key — use PaperModeExchange for routing".into(),
        )))
    }

    async fn ping(&self) -> VirsResult<bool> {
        Err(VirsError::Exchange(ExchangeError::NotSupported(
            "PaperExchange does not support ping — use PaperModeExchange for routing".into(),
        )))
    }

    async fn get_api_restrictions(&self) -> VirsResult<ApiRestrictions> {
        Err(VirsError::Exchange(ExchangeError::NotSupported(
            "PaperExchange does not support get_api_restrictions — use PaperModeExchange for routing".into(),
        )))
    }

    /* 模拟模式固定返回双向持仓（Hedge），与真实交易所要求一致 */
    async fn get_position_mode(&self) -> VirsResult<PositionMode> {

        Ok(PositionMode::Hedge)
    }


    async fn get_balance(&self) -> VirsResult<Balance> {
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
                symbol.is_none_or(|s| pos.symbol == s) && pos.quantity.abs() > 1e-8
            })
            .map(|e| {
                let pos = e.value();
                ExchangePosition {
                    symbol: pos.symbol.clone(),
                    side: pos.side.clone(),
                    quantity: pos.quantity,
                    entry_price: pos.entry_price,
                    margin_mode: pos.margin_mode,
                    info: pos.info.clone(),
                }
            })
            .collect())
    }

    async fn place_order(&self, params: PlaceOrderParams) -> VirsResult<OrderResult> {
        let order_id = self.next_order_id();
        /* 无价格或市价单类型时按市价单处理，否则按限价单处理 */
        let is_market = params.order_type == OrderType::Market || params.price.is_none();

        let client_order_id = params.client_order_id.clone().ok_or_else(|| {
            VirsError::config(format!(
                "client_order_id is required for place_order on {} — \
                 all callers must generate one via client_order_id::format_* before placing orders",
                params.symbol
            ))
        })?;

        if is_market {
            /* 市价单：用最新价格立即成交 */
            let fill_price = self
                .last_prices
                .get(&params.symbol)
                .map(|r| *r)
                .ok_or_else(|| {
                    VirsError::Exchange(ExchangeError::no_data(format!(
                        "No last price for paper market order on {}",
                        params.symbol
                    )))
                })?;
            let pending_for_fill = PaperPendingOrder {
                id: order_id,
                symbol: params.symbol.clone(),
                side: params.side.clone(),
                order_type: params.order_type.clone(),
                amount: params.amount,
                price: Some(fill_price),
                position_side: params.position_side.clone(),
                client_order_id: client_order_id.clone(),
            };
            let realized_pnl = self
                .update_position_on_fill(&pending_for_fill, fill_price)
                .await?;

            /* 市价单手续费率 0.05%（taker 费率） */
            let fee = fill_price * params.amount * 0.0005;

            let order_result = OrderResult {
                order_id: order_id.to_string(),
                client_order_id: client_order_id.clone(),
            };

            let ccxt_order = CcxtOrder {
                order_id,
                client_order_id,
                symbol: params.symbol.clone(),
                side: params.side,
                order_type: params.order_type.clone(),
                position_side: params
                    .position_side
                    .expect("engine always sends Some(Long/Short) to paper exchange"),
                original_order_type: params.order_type.clone(),
                status: CcxtOrderStatus::Filled,
                execution_type: ExecutionType::Trade,
                orig_qty: params.amount.to_string(),
                original_price: params.price.map(|p| p.to_string()).unwrap_or_else(|| "0".to_string()),
                avg_fill_price: fill_price.to_string(),
                filled_qty: params.amount.to_string(),
                last_fill_qty: params.amount.to_string(),
                last_fill_price: fill_price.to_string(),
                stop_price: "0".to_string(),
                commission: fee.to_string(),
                commission_asset: "USDT".to_string(),
                realized_pnl: realized_pnl.to_string(),
                reduce_only: false,
                is_maker: false,
                close_position: None,
                time_in_force: "GTC".to_string(),
                working_type: "CONTRACT_PRICE".to_string(),
                bids_notional: "0".to_string(),
                ask_notional: "0".to_string(),
                activation_price: None,
                callback_rate: None,
                price_protection: false,
                stp_mode: "NONE".to_string(),
                price_match_mode: "NONE".to_string(),
                gtd_auto_cancel_time: 0,
                expiry_reason: "0".to_string(),
                si: Some(0),
                ss: Some(0),
                trade_time: chrono::Utc::now().timestamp_millis(),
                trade_id: self.next_trade_id(),
                modify_id: None,
                envelope_event_type: "ORDER_TRADE_UPDATE".to_string(),
                envelope_event_time: chrono::Utc::now().timestamp_millis(),
                envelope_transaction_time: chrono::Utc::now().timestamp_millis(),
            };

            let tx = self.price_tx.lock().await;
            if let Some(ref tx) = *tx {
                if tx
                    .send(WsFeedEvent::OrderUpdate { order: Arc::new(ccxt_order) })
                    .await
                    .is_err()
                {
                    warn!(order_id = %order_id, symbol = %params.symbol, "Paper WsFeedEvent::OrderUpdate send failed — receiver dropped, event lost");
                }
            }
            Ok(order_result)
        } else {
            /* 限价单：存入挂单池，等待价格回调时触发撮合 */
            let pending = PaperPendingOrder {
                id: order_id,
                symbol: params.symbol.clone(),
                side: params.side,
                order_type: params.order_type,
                amount: params.amount,
                price: params.price,
                position_side: params.position_side,
                client_order_id: client_order_id.clone(),
            };
            self.pending.insert(order_id, pending);

            let order_result = OrderResult {
                order_id: order_id.to_string(),
                client_order_id,
            };
            Ok(order_result)
        }
    }

    async fn cancel_order(&self, _symbol: &str, order_id: &str) -> VirsResult<OrderResult> {
        let id = order_id
            .parse::<i64>()
            .map_err(|_| ExchangeError::Internal(format!("Invalid order ID: {}", order_id)))?;
        match self.pending.remove(&id) {
            Some((_, pending)) => Ok(OrderResult {
                order_id: order_id.to_string(),
                client_order_id: pending.client_order_id,
            }),
            None => Err(VirsError::Exchange(ExchangeError::OrderNotFound(
                order_id.to_string(),
            ))),
        }
    }

    async fn cancel_all_orders(&self, symbol: Option<&str>) -> VirsResult<Vec<OrderResult>> {
        let keys: Vec<i64> = self
            .pending
            .iter()
            .filter(|e| symbol.is_none_or(|s| e.value().symbol == s))
            .map(|e| *e.key())
            .collect();
        let mut canceled = Vec::new();
        for key in keys {
            if let Some((_, pending)) = self.pending.remove(&key) {
                canceled.push(OrderResult {
                    order_id: key.to_string(),
                    client_order_id: pending.client_order_id,
                });
            }
        }
        Ok(canceled)
    }

    async fn set_leverage(&self, symbol: &str, leverage: u32) -> VirsResult<()> {
        self.configured_leverage
            .insert(symbol.to_string(), leverage);
        Ok(())
    }

    async fn subscribe_order_updates(&self, _symbols: &[&str]) -> VirsResult<OrderUpdateStream> {
        let (tx, rx) = mpsc::channel(256);
        let mut price_tx = self.price_tx.lock().await;
        *price_tx = Some(tx);
        Ok(Box::pin(ReceiverStream::new(rx)))
    }

    async fn on_price_tick(&self, symbol: &str, price: f64) {
        PaperExchangeAdapter::on_price_tick(self, symbol, price).await;
    }

    async fn restore_positions(&self, positions: Vec<ExchangePosition>) {
        for pos in positions {
            if pos.quantity.abs() < 1e-8 {
                continue;
            }
            let key = format!("{}:{:?}", pos.symbol, pos.side);
            self.positions.insert(
                key.clone(),
                PaperPosition {
                    symbol: pos.symbol.clone(),
                    side: pos.side,
                    quantity: pos.quantity,
                    entry_price: pos.entry_price,
                    margin_mode: pos.margin_mode,
                    info: pos.info.clone(),
                },
            );

            self.last_prices.insert(pos.symbol.clone(), pos.entry_price);
        }
    }
}
