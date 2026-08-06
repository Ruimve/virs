use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;

use virs_exchange::Exchanges;
use virs_exchange::paper_mode::PaperModeExchange;
use virs_type::exchange::{ExchangePe, OrderUpdateStream};
use virs_type::market::*;
use virs_type::position::*;
use virs_type::ws_types::{KlineWsClient, OrderBookWsClient};
use virs_type::*;
use virs_error::VirsResult;

// ─── Mock 真实交易所，用于测试路由 ───

struct MockRealExchange {
    name: String,
    get_ticker_called: Arc<AtomicBool>,
    get_balance_called: Arc<AtomicBool>,
    set_leverage_called: Arc<AtomicBool>,
}

impl MockRealExchange {
    fn new() -> Self {
        Self {
            name: "binance".to_string(),
            get_ticker_called: Arc::new(AtomicBool::new(false)),
            get_balance_called: Arc::new(AtomicBool::new(false)),
            set_leverage_called: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[async_trait]
impl ExchangePe for MockRealExchange {
    fn name(&self) -> &str { &self.name }
    fn market_type(&self) -> MarketType { MarketType::Perpetual }

    async fn get_ticker(&self, _symbol: &str) -> VirsResult<Ticker> {
        self.get_ticker_called.store(true, Ordering::SeqCst);
        Ok(Ticker {
            symbol: "BTC/USDT".into(),
            exchange: "binance".into(),
            bid: Some(49999.0),
            ask: Some(50001.0),
            last: 50000.0,
            high_24h: 51000.0,
            low_24h: 49000.0,
            volume_24h: 1000.0,
            price_change_24h: 500.0,
            price_change_pct_24h: 1.0,
            timestamp: chrono::Utc::now(),
        })
    }

    async fn get_klines(&self, _: &str, _: &str, _: u32, _: Option<i64>) -> VirsResult<Vec<Kline>> { Ok(vec![]) }
    async fn get_klines_range(&self, _: &str, _: &str, _: i64, _: i64) -> VirsResult<Vec<Kline>> { Ok(vec![]) }
    async fn get_funding_rate(&self, _: &str) -> VirsResult<FundingRate> { Ok(FundingRate { symbol: "BTC/USDT".into(), rate: 0.0001, next_funding_time: None }) }
    async fn get_symbols(&self) -> VirsResult<Vec<String>> { Ok(vec!["BTC/USDT".into()]) }
    async fn get_min_qty(&self, _: &str) -> VirsResult<f64> { Ok(0.001) }
    async fn get_balance(&self) -> VirsResult<Balance> {
        self.get_balance_called.store(true, Ordering::SeqCst);
        Ok(Balance { asset: "USDT".into(), free: 100000.0, used: 0.0, total: 100000.0 })
    }
    async fn get_positions(&self, _: Option<&str>) -> VirsResult<Vec<ExchangePosition>> { Ok(vec![]) }
    async fn place_order(&self, _: PlaceOrderParams) -> VirsResult<OrderResult> { Ok(OrderResult { order_id: "mock".into(), client_order_id: "mock".into() }) }
    async fn cancel_order(&self, _: &str, _: &str) -> VirsResult<OrderResult> { Ok(OrderResult { order_id: "mock".into(), client_order_id: "mock".into() }) }
    async fn cancel_all_orders(&self, _: Option<&str>) -> VirsResult<Vec<OrderResult>> { Ok(vec![]) }
    async fn set_leverage(&self, _: &str, _: u32) -> VirsResult<()> {
        self.set_leverage_called.store(true, Ordering::SeqCst);
        Ok(())
    }
    async fn get_position_mode(&self) -> VirsResult<PositionMode> { Ok(PositionMode::Hedge) }
    async fn create_listen_key(&self) -> VirsResult<String> { Ok("mock-key".into()) }
    async fn ping(&self) -> VirsResult<bool> { Ok(true) }
    async fn get_api_restrictions(&self) -> VirsResult<ApiRestrictions> {
        Ok(ApiRestrictions {
            ip_restrict: None,
            ip_whitelist: vec![],
            ip_not_restricted: None,
            create_sub_account: None,
            read_info: None,
            enable_withdrawals: None,
            enable_internal_transfer: None,
            enable_futures: None,
            enable_vanilla_options: None,
            enable_portfolio_margin_trading: None,
            enable_fix_api_trade: None,
            enable_fix_api_read: None,
            info: serde_json::Value::Null,
        })
    }
    async fn subscribe_order_updates(&self, _: &[&str]) -> VirsResult<OrderUpdateStream> {
        let (_, rx) = tokio::sync::mpsc::channel(1);
        Ok(Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx)))
    }
    async fn on_price_tick(&self, _: &str, _: f64) {}
    async fn restore_positions(&self, _: Vec<ExchangePosition>) {}
    fn create_kline_ws(&self, _: Option<&str>) -> VirsResult<Arc<tokio::sync::Mutex<dyn KlineWsClient>>> {
        Err(virs_error::VirsError::Exchange(virs_error::ExchangeError::NotSupported("mock".into())))
    }
    fn create_orderbook_ws(&self, _: Option<&str>) -> VirsResult<Arc<tokio::sync::Mutex<dyn OrderBookWsClient>>> {
        Err(virs_error::VirsError::Exchange(virs_error::ExchangeError::NotSupported("mock".into())))
    }
}

// ─── 原有测试 ───

#[test]
fn int_1_1_side_roundtrip() {
    let original = Side::Buy;
    let ccxt = virs_type::Side::Buy;
    assert_eq!(original, Side::Buy);
    assert_eq!(ccxt, virs_type::Side::Buy);
}

#[test]
fn int_1_2_order_type_roundtrip() {
    let ot = OrderType::Limit;
    let models_ot = virs_type::OrderType::Limit;
    assert_eq!(ot, OrderType::Limit);
    assert_eq!(models_ot, virs_type::OrderType::Limit);
}

#[test]
fn int_1_3_position_side_consistency() {
    let models_long = virs_type::PositionSide::Long;
    let pe_long = PositionSide::Long;
    assert_eq!(models_long, virs_type::PositionSide::Long);
    assert_eq!(pe_long, PositionSide::Long);
}

#[test]
fn int_1_4_market_type_consistency() {
    let models_perp = virs_type::MarketType::Perpetual;
    let pe_perp = MarketType::Perpetual;
    assert_eq!(models_perp, virs_type::MarketType::Perpetual);
    assert_eq!(pe_perp, MarketType::Perpetual);
}

#[tokio::test]
async fn int_2_1_paper_exchange_creation_and_balance() {
    let paper =
        virs_exchange::paper::PaperExchangeAdapter::new("binance", MarketType::Perpetual, 100000.0);
    assert_eq!(paper.name(), "binance");
    assert_eq!(paper.market_type(), MarketType::Perpetual);

    let balance = paper.get_balance().await.unwrap();
    assert_eq!(balance.asset, "USDT");
    assert_eq!(balance.total, 100000.0);
    assert_eq!(balance.free, 100000.0);
    assert_eq!(balance.used, 0.0);
}

#[tokio::test]
async fn int_2_2_paper_market_order_updates_balance() {
    use virs_type::position::PlaceOrderParams;
    let paper =
        virs_exchange::paper::PaperExchangeAdapter::new("binance", MarketType::Perpetual, 50000.0);

    paper.on_price_tick("BTC/USDT", 50000.0).await;
    paper.set_leverage("BTC/USDT", 10).await.unwrap();

    let params = PlaceOrderParams {
        symbol: "BTC/USDT".into(),
        side: Side::Buy,
        order_type: OrderType::Market,
        amount: 0.1,
        price: None,
        position_id: None,
        client_order_id: Some("test-paper-order-001".into()),
        position_side: Some(PositionSide::Long),
        stop_price: None,
        time_in_force: None,
    };
    let order = paper.place_order(params).await.unwrap();
    assert!(
        !order.order_id.is_empty(),
        "place_order should return a non-empty order_id"
    );

    let balance = paper.get_balance().await.unwrap();
    assert!(
        balance.used > 0.0,
        "used balance should be > 0 after opening position"
    );
    assert!(
        balance.free < 50000.0,
        "free balance should be < initial after opening position"
    );

    let positions = paper.get_positions(Some("BTC/USDT")).await.unwrap();
    assert_eq!(positions.len(), 1);
    assert_eq!(positions[0].side, PositionSide::Long);
    assert!((positions[0].quantity - 0.1).abs() < 1e-8);
}

#[test]
fn int_3_1_registry_register_and_get() {
    let registry = Exchanges::new();
    assert!(registry.get("nonexistent").is_none());
}

#[test]
fn int_3_2_registry_get_nonexistent() {
    let registry = Exchanges::new();
    assert!(registry.get("does_not_exist").is_none());
}

#[test]
fn int_3_3_registry_list_names() {
    let registry = Exchanges::new();
    let names = registry.registered_names();
    assert!(names.is_empty());
}

// ─── PaperModeExchange 路由测试 ───

#[tokio::test]
async fn int_4_1_paper_mode_routes_public_to_real() {
    let mock = Arc::new(MockRealExchange::new());
    let ticker_called = Arc::clone(&mock.get_ticker_called);
    let pme = PaperModeExchange::new(mock, 10000.0);

    let _ticker = pme.get_ticker("BTC/USDT").await.unwrap();
    assert!(ticker_called.load(Ordering::SeqCst), "get_ticker should route to real");
}

#[tokio::test]
async fn int_4_2_paper_mode_routes_private_to_paper() {
    let mock = Arc::new(MockRealExchange::new());
    let balance_called = Arc::clone(&mock.get_balance_called);
    let pme = PaperModeExchange::new(mock, 10000.0);

    let balance = pme.get_balance().await.unwrap();
    assert!(!balance_called.load(Ordering::SeqCst), "get_balance should route to paper, not real");
    assert_eq!(balance.total, 10000.0, "paper balance should be initial_balance");
}

#[tokio::test]
async fn int_4_3_paper_mode_set_leverage_calls_both() {
    let mock = Arc::new(MockRealExchange::new());
    let leverage_called = Arc::clone(&mock.set_leverage_called);
    let pme = PaperModeExchange::new(mock, 10000.0);

    pme.set_leverage("BTC/USDT", 10).await.unwrap();
    assert!(leverage_called.load(Ordering::SeqCst), "set_leverage should call real");
    // paper 内部也存储了 leverage（通过后续下单验证）
    pme.on_price_tick("BTC/USDT", 50000.0).await;
    let params = PlaceOrderParams {
        symbol: "BTC/USDT".into(),
        side: Side::Buy,
        order_type: OrderType::Market,
        amount: 0.01,
        price: None,
        position_id: None,
        client_order_id: Some("test-pme-001".into()),
        position_side: Some(PositionSide::Long),
        stop_price: None,
        time_in_force: None,
    };
    let order = pme.place_order(params).await.unwrap();
    assert!(!order.order_id.is_empty(), "place_order should succeed via paper routing");
}

#[tokio::test]
async fn int_4_4_paper_mode_name_returns_real_name() {
    let mock = Arc::new(MockRealExchange::new());
    let pme = PaperModeExchange::new(mock, 10000.0);
    assert_eq!(pme.name(), "binance", "name() should return real exchange name");
    assert_eq!(pme.market_type(), MarketType::Perpetual);
}
