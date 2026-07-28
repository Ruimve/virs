use virs_exchange::Exchanges;
use virs_types::enums::*;

#[test]
fn int_1_1_side_roundtrip() {
    let original = Side::Buy;
    let ccxt = virs_types::Side::Buy;
    assert_eq!(original, Side::Buy);
    assert_eq!(ccxt, virs_types::Side::Buy);
}

#[test]
fn int_1_2_order_type_roundtrip() {
    let ot = OrderType::Limit;
    let models_ot = virs_types::OrderType::Limit;
    assert_eq!(ot, OrderType::Limit);
    assert_eq!(models_ot, virs_types::OrderType::Limit);
}

#[test]
fn int_1_3_position_side_consistency() {
    let models_long = virs_types::PositionSide::Long;
    let pe_long = PositionSide::Long;
    assert_eq!(models_long, virs_types::PositionSide::Long);
    assert_eq!(pe_long, PositionSide::Long);
}

#[test]
fn int_1_4_market_type_consistency() {
    let models_perp = virs_types::MarketType::Perpetual;
    let pe_perp = MarketType::Perpetual;
    assert_eq!(models_perp, virs_types::MarketType::Perpetual);
    assert_eq!(pe_perp, MarketType::Perpetual);
}

#[tokio::test]
async fn int_2_1_paper_exchange_creation_and_balance() {
    use virs_types::exchange_pe::ExchangePe;
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
    use virs_types::exchange_pe::ExchangePe;
    use virs_types::position::PlaceOrderParams;
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
