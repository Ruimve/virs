use chrono::Utc;

use virs_exchange::Exchanges;
use virs_models as models;
use virs_types::enums::*;

use virs_error::ExchangeError;


#[test]
fn int_1_1_side_roundtrip() {

    let original = Side::Buy;
    let ccxt = virs_ccxt::Side::Buy;
    assert_eq!(original, Side::Buy);
    assert_eq!(ccxt, virs_ccxt::Side::Buy);
}

#[test]
fn int_1_2_order_type_roundtrip() {

    let ot = OrderType::Limit;
    let models_ot = models::OrderType::Limit;
    assert_eq!(ot, OrderType::Limit);
    assert_eq!(models_ot, models::OrderType::Limit);
}

#[test]
fn int_1_3_position_side_consistency() {

    let models_long = models::PositionSide::Long;
    let pe_long = PositionSide::Long;
    assert_eq!(models_long, models::PositionSide::Long);
    assert_eq!(pe_long, PositionSide::Long);
}

#[test]
fn int_1_4_market_type_consistency() {
    let models_perp = models::MarketType::Perpetual;
    let pe_perp = MarketType::Perpetual;
    assert_eq!(models_perp, models::MarketType::Perpetual);
    assert_eq!(pe_perp, MarketType::Perpetual);
}


#[tokio::test]
async fn int_2_1_paper_exchange_creation_and_balance() {
    use virs_types::exchange_pe::ExchangePe;
    let paper = virs_exchange::paper::PaperExchangeAdapter::new(
        "binance",
        MarketType::Perpetual,
        100000.0,
    );
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
    use virs_types::exchange_pe::ExchangePe;
    use virs_types::position::PlaceOrderParams;
    let paper = virs_exchange::paper::PaperExchangeAdapter::new(
        "binance",
        MarketType::Perpetual,
        50000.0,
    );


    paper.on_price_tick("BTC/USDT", 50000.0).await;

    let params = PlaceOrderParams {
        symbol: "BTC/USDT".into(),
        side: Side::Buy,
        order_type: OrderType::Market,
        amount: 0.1,
        price: None,
        position_id: None,
        client_order_id: None,
        reduce_only: false,
        position_side: Some(PositionSide::Long),
    };
    let order = paper.place_order(params).await.unwrap();
    assert_eq!(order.status, OrderStatus::Filled);
    assert_eq!(order.filled, 0.1);


    let balance = paper.get_balance().await.unwrap();
    assert!(balance.used > 0.0, "used balance should be > 0 after opening position");
    assert!(balance.free < 50000.0, "free balance should be < initial after opening position");


    let positions = paper.get_positions(Some("BTC/USDT")).await.unwrap();
    assert_eq!(positions.len(), 1);
    assert_eq!(positions[0].side, PositionSide::Long);
    assert!((positions[0].size - 0.1).abs() < 1e-8);
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


#[test]
fn int_4_1_no_exchange_error() {
    let err = virs_exchange::pe_adapter::no_exchange_error();
    match err {
        ExchangeError::Internal(msg) => {
            assert!(msg.contains("No perpetual exchange"));
        }
        _ => panic!("Expected Internal variant"),
    }
}


#[test]
fn int_6_1_order_conversion_full_chain() {
    let now = Utc::now();
    let mo = models::Order {
        id: "binance_order_123".into(),
        client_order_id: Some("client_abc".into()),
        symbol: "ETH/USDT".into(),
        side: models::Side::Buy,
        order_type: models::OrderType::Limit,
        price: Some(3000.0),
        amount: 10.0,
        cost: Some(30000.0),
        filled: 5.0,
        remaining: 5.0,
        status: models::OrderStatus::PartiallyFilled,
        fee: 1.5,
        fee_currency: "USDT".into(),
        created_at: now,
        updated_at: now,
    };

    let po = virs_exchange::pe_adapter::convert_order(&mo, "binance");
    assert_eq!(po.exchange_order_id, Some("binance_order_123".into()));
    assert_eq!(po.client_order_id, Some("client_abc".into()));
    assert_eq!(po.exchange, "binance");
    assert_eq!(po.symbol, "ETH/USDT");
    assert_eq!(po.side, Side::Buy);
    assert_eq!(po.order_type, OrderType::Limit);
    assert_eq!(po.request_price, Some(3000.0));
    assert_eq!(po.fill_price, Some(3000.0));
    assert_eq!(po.amount, 10.0);
    assert_eq!(po.filled, 5.0);
    assert_eq!(po.remaining, 5.0);
    assert_eq!(po.status, OrderStatus::PartiallyFilled);
    assert_eq!(po.fee, 1.5);
    assert_eq!(po.fee_currency, "USDT");
}

#[test]
fn int_6_2_position_conversion_full_chain() {
    let ep = models::ExchangePosition {
        symbol: "BTC/USDT".into(),
        side: models::PositionSide::Short,
        size: 2.0,
        entry_price: 45000.0,
        leverage: 20,
        unrealized_pnl: -500.0,
        liquidation_price: Some(47000.0),
    };

    let result = virs_exchange::pe_adapter::convert_exchange_position(&ep);
    assert_eq!(result.symbol, "BTC/USDT");
    assert_eq!(result.side, PositionSide::Short);
    assert_eq!(result.size, 2.0);
    assert_eq!(result.entry_price, 45000.0);
    assert_eq!(result.leverage, 20);
    assert_eq!(result.unrealized_pnl, -500.0);
    assert_eq!(result.liquidation_price, Some(47000.0));
}
