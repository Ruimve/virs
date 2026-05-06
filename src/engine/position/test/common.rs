use crate::engine::position::config::EngineConfig;
use crate::engine::position::error::{PositionEngineError, Result};
use crate::engine::position::exchange::Exchange;
use crate::engine::position::types::*;
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

pub fn make_position(
    symbol: &str,
    size: f64,
    entry_price: f64,
    leverage: u32,
) -> Position {
    Position {
        id: Uuid::new_v4(),
        engine_id: "test-engine".to_string(),
        strategy_id: None,
        exchange: "binance".to_string(),
        symbol: symbol.to_string(),
        side: PositionSide::Long,
        status: PositionStatus::Open,
        size,
        entry_price,
        current_price: entry_price,
        leverage,
        margin: size * entry_price / leverage as f64,
        unrealized_pnl: 0.0,
        realized_pnl: 0.0,
        stop_loss: None,
        take_profit: None,
        liquidation_price: None,
        opened_at: Utc::now(),
        updated_at: Utc::now(),
        closed_at: None,
        metadata: serde_json::Value::Null,
    }
}

pub fn make_position_with_liquidation(
    symbol: &str,
    size: f64,
    entry_price: f64,
    leverage: u32,
    liquidation_price: Option<f64>,
    current_price: f64,
) -> Position {
    let mut pos = make_position(symbol, size, entry_price, leverage);
    pos.liquidation_price = liquidation_price;
    pos.current_price = current_price;
    pos
}

pub fn make_position_side(
    symbol: &str,
    side: PositionSide,
    size: f64,
    entry_price: f64,
    leverage: u32,
) -> Position {
    let mut pos = make_position(symbol, size, entry_price, leverage);
    pos.side = side;
    pos
}

pub fn make_position_with_id(
    id: Uuid,
    symbol: &str,
    side: PositionSide,
    size: f64,
    entry_price: f64,
    leverage: u32,
) -> Position {
    let mut pos = make_position_side(symbol, side, size, entry_price, leverage);
    pos.id = id;
    pos
}

pub fn make_trade(
    position_id: Uuid,
    order_id: Uuid,
    side: Side,
    price: f64,
    amount: f64,
    trade_type: &str,
) -> Trade {
    Trade {
        id: Uuid::new_v4(),
        position_id,
        order_id,
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side,
        price,
        amount,
        fee: price * amount * 0.0005,
        fee_currency: "USDT".to_string(),
        pnl: 0.0,
        trade_type: trade_type.to_string(),
        created_at: Utc::now(),
    }
}

pub fn make_engine_config() -> EngineConfig {
    EngineConfig {
        engine_id: "test_engine".to_string(),
        sync_interval_secs: 1,
        poll_interval_secs: 1,
        ws_reconnect_timeout_secs: 1,
        risk: crate::engine::position::config::RiskConfig::default(),
        pnl_snapshot_interval_secs: 1,
    }
}

pub fn make_order(
    symbol: &str,
    side: Side,
    order_type: OrderType,
    amount: f64,
    price: Option<f64>,
) -> Order {
    Order {
        id: Uuid::new_v4(),
        position_id: Uuid::nil(),
        exchange_order_id: Some(format!("ex_{}", Uuid::new_v4())),
        client_order_id: None,
        exchange: "binance".to_string(),
        symbol: symbol.to_string(),
        side,
        order_type,
        request_price: price,
        fill_price: None,
        amount,
        filled: 0.0,
        remaining: amount,
        status: OrderStatus::Open,
        reduce_only: false,
        fee: 0.0,
        fee_currency: "USDT".to_string(),
        slippage: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

pub fn make_filled_order(
    symbol: &str,
    side: Side,
    amount: f64,
    fill_price: f64,
) -> Order {
    Order {
        id: Uuid::new_v4(),
        position_id: Uuid::nil(),
        exchange_order_id: Some(format!("ex_{}", Uuid::new_v4())),
        client_order_id: None,
        exchange: "binance".to_string(),
        symbol: symbol.to_string(),
        side,
        order_type: OrderType::Market,
        request_price: None,
        fill_price: Some(fill_price),
        amount,
        filled: amount,
        remaining: 0.0,
        status: OrderStatus::Filled,
        reduce_only: false,
        fee: fill_price * amount * 0.0005,
        fee_currency: "USDT".to_string(),
        slippage: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

struct MockExchangeInner {
    tickers: HashMap<String, Ticker>,
    balance: Balance,
    positions: Vec<ExchangePosition>,
    place_order_ok: Option<Order>,
    place_order_fail: bool,
    place_order_error_msg: String,
    cancel_order_ok: Option<Order>,
    cancel_all_ok: Option<Vec<Order>>,
    open_orders: Vec<Order>,
    place_order_count: usize,
    cancel_order_count: usize,
    leverage_log: Vec<(String, u32)>,
    get_positions_fail: bool,
}

pub struct MockExchange {
    name: String,
    inner: Arc<tokio::sync::Mutex<MockExchangeInner>>,
}

impl MockExchange {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            inner: Arc::new(tokio::sync::Mutex::new(MockExchangeInner {
                tickers: HashMap::new(),
                balance: Balance {
                    asset: "USDT".to_string(),
                    free: 10000.0,
                    used: 0.0,
                    total: 10000.0,
                },
                positions: Vec::new(),
                place_order_ok: None,
                place_order_fail: false,
                place_order_error_msg: String::new(),
                cancel_order_ok: None,
                cancel_all_ok: None,
                open_orders: Vec::new(),
                place_order_count: 0,
                cancel_order_count: 0,
                leverage_log: Vec::new(),
                get_positions_fail: false,
            })),
        }
    }

    pub async fn set_ticker(&self, symbol: &str, price: f64) {
        let ticker = Ticker {
            symbol: symbol.to_string(),
            price,
            bid: price * 0.999,
            ask: price * 1.001,
            volume_24h: 1000000.0,
            timestamp: Utc::now(),
        };
        self.inner.lock().await.tickers.insert(symbol.to_string(), ticker);
    }

    pub async fn set_balance(&self, total: f64) {
        let mut inner = self.inner.lock().await;
        inner.balance.total = total;
        inner.balance.free = total;
    }

    pub async fn set_positions(&self, positions: Vec<ExchangePosition>) {
        self.inner.lock().await.positions = positions;
    }

    pub async fn set_place_order_ok(&self, order: Order) {
        let mut inner = self.inner.lock().await;
        inner.place_order_ok = Some(order);
        inner.place_order_fail = false;
    }

    pub async fn set_place_order_err(&self, err: PositionEngineError) {
        let mut inner = self.inner.lock().await;
        inner.place_order_error_msg = err.to_string();
        inner.place_order_fail = true;
    }

    pub async fn set_cancel_order_ok(&self, order: Order) {
        self.inner.lock().await.cancel_order_ok = Some(order);
    }

    pub async fn set_cancel_all_ok(&self, orders: Vec<Order>) {
        self.inner.lock().await.cancel_all_ok = Some(orders);
    }

    pub async fn set_open_orders(&self, orders: Vec<Order>) {
        self.inner.lock().await.open_orders = orders;
    }

    pub async fn place_order_count(&self) -> usize {
        self.inner.lock().await.place_order_count
    }

    pub async fn cancel_order_count(&self) -> usize {
        self.inner.lock().await.cancel_order_count
    }

    pub async fn leverage_log(&self) -> Vec<(String, u32)> {
        self.inner.lock().await.leverage_log.clone()
    }

    pub async fn set_get_positions_fail(&self, fail: bool) {
        self.inner.lock().await.get_positions_fail = fail;
    }
}

impl Clone for MockExchange {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            inner: self.inner.clone(),
        }
    }
}

#[async_trait]
impl Exchange for MockExchange {
    fn name(&self) -> &str {
        &self.name
    }

    fn market_type(&self) -> MarketType {
        MarketType::Perpetual
    }

    async fn get_ticker(&self, symbol: &str) -> Result<Ticker> {
        self.inner
            .lock()
            .await
            .tickers
            .get(symbol)
            .cloned()
            .ok_or_else(|| PositionEngineError::Exchange(format!("Ticker not found: {}", symbol)))
    }

    async fn get_balance(&self) -> Result<Balance> {
        Ok(self.inner.lock().await.balance.clone())
    }

    async fn get_positions(&self, _symbol: Option<&str>) -> Result<Vec<ExchangePosition>> {
        let inner = self.inner.lock().await;
        if inner.get_positions_fail {
            Err(PositionEngineError::Exchange("get_positions failed".to_string()))
        } else {
            Ok(inner.positions.clone())
        }
    }

    async fn get_funding_rate(&self, symbol: &str) -> Result<FundingRate> {
        Ok(FundingRate {
            symbol: symbol.to_string(),
            rate: 0.0001,
            next_funding_time: Utc::now(),
        })
    }

    async fn get_fee_rates(&self, symbol: &str) -> Result<FeeRates> {
        Ok(FeeRates {
            symbol: symbol.to_string(),
            maker_rate: 0.0002,
            taker_rate: 0.0005,
        })
    }

    async fn place_order(&self, params: PlaceOrderParams) -> Result<Order> {
        let mut inner = self.inner.lock().await;
        inner.place_order_count += 1;
        if inner.place_order_fail {
            let msg = inner.place_order_error_msg.clone();
            return Err(PositionEngineError::Exchange(msg));
        }
        match &inner.place_order_ok {
            Some(order) => Ok(order.clone()),
            None => {
                let order = Order {
                    id: Uuid::new_v4(),
                    position_id: Uuid::nil(),
                    exchange_order_id: Some(format!("ex_{}", Uuid::new_v4())),
                    client_order_id: None,
                    exchange: self.name.clone(),
                    symbol: params.symbol.clone(),
                    side: params.side,
                    order_type: params.order_type,
                    request_price: params.price,
                    fill_price: params.price.or(Some(50000.0)),
                    amount: params.amount,
                    filled: params.amount,
                    remaining: 0.0,
                    status: OrderStatus::Filled,
                    reduce_only: params.reduce_only,
                    fee: 0.1,
                    fee_currency: "USDT".to_string(),
                    slippage: None,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                Ok(order)
            }
        }
    }

    async fn cancel_order(&self, _symbol: &str, _order_id: &str) -> Result<Order> {
        let mut inner = self.inner.lock().await;
        inner.cancel_order_count += 1;
        match &inner.cancel_order_ok {
            Some(order) => Ok(order.clone()),
            None => Err(PositionEngineError::Exchange("cancel not configured".to_string())),
        }
    }

    async fn cancel_all_orders(&self, _symbol: &str) -> Result<Vec<Order>> {
        let inner = self.inner.lock().await;
        match &inner.cancel_all_ok {
            Some(orders) => Ok(orders.clone()),
            None => Ok(vec![]),
        }
    }

    async fn get_open_orders(&self, _symbol: Option<&str>) -> Result<Vec<Order>> {
        Ok(self.inner.lock().await.open_orders.clone())
    }

    async fn get_order(&self, _symbol: &str, _order_id: &str) -> Result<Order> {
        Err(PositionEngineError::Exchange("get_order not implemented".to_string()))
    }

    async fn set_leverage(&self, symbol: &str, leverage: u32) -> Result<()> {
        self.inner.lock().await.leverage_log.push((symbol.to_string(), leverage));
        Ok(())
    }

    async fn subscribe_order_updates(&self, _symbols: &[&str]) -> Result<mpsc::Receiver<WsFeedEvent>> {
        let (_, rx) = mpsc::channel(16);
        Ok(rx)
    }
}
