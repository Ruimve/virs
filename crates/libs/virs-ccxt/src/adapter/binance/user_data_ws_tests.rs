use crate::adapter::binance::user_data_ws::*;
use crate::adapter::binance::BinanceSigner;
use crate::ExchangeClient;
use std::sync::Arc;
use virs_types::{CcxtOrder, CcxtOrderStatus, OrderStatus, PositionSide};


#[test]
fn test_parse_invalid_json() {
    let result: Result<BinanceOrderMessage, _> = serde_json::from_str("not json");
    assert!(result.is_err());
}

#[test]
fn test_parse_non_order_event() {

    let json = r#"{
        "e": "ACCOUNT_UPDATE",
        "E": 1713900000000,
        "T": 1713900000000
    }"#;

    let msg: BinanceOrderMessage = serde_json::from_str(json).unwrap();
    let event = msg.to_ws_feed_event();

    assert!(event.is_none());
}

#[test]
fn test_parse_listen_key_expired() {

    let json = r#"{
        "e": "listenKeyExpired",
        "E": 1713900000000
    }"#;

    let msg: BinanceOrderMessage = serde_json::from_str(json).unwrap();
    let event = msg.to_ws_feed_event();
    assert!(event.is_none());
}

#[test]
fn test_parse_order_trade_update_single_stream() {


    let json = r#"{
        "e": "ORDER_TRADE_UPDATE",
        "E": 1713900000000,
        "T": 1713900000123,
        "o": {
            "s": "BTCUSDT",
            "c": "test_client",
            "S": "SELL",
            "o": "LIMIT",
            "f": "GTC",
            "q": "0.002",
            "p": "65000.00",
            "ap": "65000.00",
            "x": "FILLED",
            "X": "FILLED",
            "i": 123456789,
            "l": "0.002",
            "z": "0.002",
            "L": "65000.00",
            "n": "0.065",
            "N": "USDT",
            "T": 1713900000123,
            "t": 1,
            "R": true,
            "w": "CONTRACT_PRICE",
            "m": false,
            "ps": "SHORT"
        }
    }"#;

    let msg: BinanceOrderMessage = serde_json::from_str(json).unwrap();


    let event = msg.to_ws_feed_event();
    assert!(
        event.is_some(),
        "ORDER_TRADE_UPDATE should produce WsFeedEvent"
    );

    if let Some(WsFeedEvent::OrderUpdate { order }) = event {
        assert_eq!(order.order_id, 123456789);
        assert_eq!(order.symbol, "BTCUSDT");
        assert_eq!(order.status, CcxtOrderStatus::Filled);
        let filled: f64 = order.filled_qty.parse().unwrap();
        assert!((filled - 0.002).abs() < 0.0001);
        let amount: f64 = order.orig_qty.parse().unwrap();
        let remaining = (amount - filled).max(0.0);
        assert!((remaining - 0.0).abs() < 0.0001);
        assert_eq!(order.position_side, PositionSide::Short);
    } else {
        panic!("Expected OrderUpdate event");
    }
}


#[test]
fn test_order_status_mapping_all_variants() {
    let cases = vec![
        ("NEW", Some(OrderStatus::Open)),
        ("PARTIALLY_FILLED", Some(OrderStatus::PartiallyFilled)),
        ("FILLED", Some(OrderStatus::Filled)),
        ("CANCELED", Some(OrderStatus::Canceled)),
        ("EXPIRED", Some(OrderStatus::Canceled)),
        ("EXPIRED_IN_MATCH", Some(OrderStatus::Canceled)),
        ("REJECTED", Some(OrderStatus::Failed)),
        ("PENDING_CANCEL", None),
    ];

    for (binance_status, expected) in cases {
        let inner = BinanceOrderInner {
            symbol: "BTCUSDT".to_string(),
            client_order_id: "test".to_string(),
            side: "BUY".to_string(),
            order_type: "LIMIT".to_string(),
            status: binance_status.to_string(),
            order_id: 1,
            orig_qty: "1.0".to_string(),
            filled_qty: "0.0".to_string(),
            remaining_qty: Some("1.0".to_string()),
            last_fill_price: "0.0".to_string(),
            avg_fill_price: None,
            last_fill_qty: "0.0".to_string(),
            commission: "0.0".to_string(),
            commission_asset: "USDT".to_string(),
            trade_time: 0,
            is_reduce_only: false,
            working_type: "CONTRACT_PRICE".to_string(),
            position_side: None,
        };
        assert_eq!(
            inner.to_order_status(),
            expected,
            "Binance status '{}' should map to {:?}",
            binance_status,
            expected
        );
    }
}


fn unwrap_order_update(event: WsFeedEvent) -> CcxtOrder {
    match event {
        WsFeedEvent::OrderUpdate { order } => order,
        WsFeedEvent::ConnectionChanged { .. } => {
            panic!("Expected OrderUpdate, got ConnectionChanged")
        }
    }
}

#[test]
fn test_to_ws_feed_event_filled() {
    let inner = make_test_inner("FILLED", "1.0", "1.0", "0.0", "65000.00", "1.0", "0.065");
    let event = inner.to_ws_feed_event().unwrap();
    let order = unwrap_order_update(event);

    assert_eq!(order.order_id, 123456789);
    assert_eq!(order.symbol, "BTCUSDT");
    assert_eq!(order.status, CcxtOrderStatus::Filled);
    let filled: f64 = order.filled_qty.parse().unwrap();
    assert!((filled - 1.0).abs() < 0.001);
    let amount: f64 = order.orig_qty.parse().unwrap();
    let remaining = (amount - filled).max(0.0);
    assert!((remaining - 0.0).abs() < 0.001);
    let price: f64 = order.last_fill_price.parse().unwrap();
    assert!((price - 65000.0).abs() < 0.001);
    assert!((amount - 1.0).abs() < 0.001);
    let commission: f64 = order.commission.parse().unwrap();
    assert!((commission - 0.065).abs() < 0.001);
}

#[test]
fn test_to_ws_feed_event_partially_filled() {
    let inner = make_test_inner(
        "PARTIALLY_FILLED",
        "10.0",
        "5.0",
        "5.0",
        "3500.00",
        "5.0",
        "0.175",
    );
    let event = inner.to_ws_feed_event().unwrap();
    let order = unwrap_order_update(event);

    assert_eq!(order.status, CcxtOrderStatus::PartiallyFilled);
    let filled: f64 = order.filled_qty.parse().unwrap();
    assert!((filled - 5.0).abs() < 0.001);
    let amount: f64 = order.orig_qty.parse().unwrap();
    let remaining = (amount - filled).max(0.0);
    assert!((remaining - 5.0).abs() < 0.001);
}

#[test]
fn test_to_ws_feed_event_new_order() {
    let inner = make_test_inner("NEW", "1.0", "0.0", "1.0", "0.00", "0.0", "0.0");
    let event = inner.to_ws_feed_event().unwrap();
    let order = unwrap_order_update(event);

    assert_eq!(order.status, CcxtOrderStatus::New);
    let filled: f64 = order.filled_qty.parse().unwrap();
    assert!((filled - 0.0).abs() < 0.001);
    let amount: f64 = order.orig_qty.parse().unwrap();
    let remaining = (amount - filled).max(0.0);
    assert!((remaining - 1.0).abs() < 0.001);
}

#[test]
fn test_to_ws_feed_event_unknown_status() {
    let inner = make_test_inner("PENDING_CANCEL", "1.0", "0.0", "1.0", "0.00", "0.0", "0.0");
    let event = inner.to_ws_feed_event();
    assert!(event.is_none(), "Unknown status should return None");
}

#[test]
fn test_to_ws_feed_event_remaining_fallback() {

    let inner = BinanceOrderInner {
        symbol: "BTCUSDT".to_string(),
        client_order_id: "test".to_string(),
        side: "BUY".to_string(),
        order_type: "LIMIT".to_string(),
        status: "PARTIALLY_FILLED".to_string(),
        order_id: 123456789,
        orig_qty: "10.0".to_string(),
        filled_qty: "3.0".to_string(),
        remaining_qty: None,
        last_fill_price: "65000.00".to_string(),
        avg_fill_price: None,
        last_fill_qty: "3.0".to_string(),
        commission: "0.195".to_string(),
        commission_asset: "USDT".to_string(),
        trade_time: 1713900000123,
        is_reduce_only: false,
        working_type: "CONTRACT_PRICE".to_string(),
        position_side: None,
    };
    let event = inner.to_ws_feed_event().unwrap();
    let order = unwrap_order_update(event);
    let amount: f64 = order.orig_qty.parse().unwrap();
    let filled: f64 = order.filled_qty.parse().unwrap();
    let remaining = (amount - filled).max(0.0);
    assert!((remaining - 7.0).abs() < 0.001, "remaining = 10 - 3 = 7");
}


#[test]
fn test_new_perpetual() {
    let client = ExchangeClient::with_api_key(
        10,
        None,
        Some("test_api_key"),
        std::time::Duration::from_secs(5),
        std::time::Duration::from_secs(3),
        10,
    )
    .expect("Failed to create test ExchangeClient");
    let signer = Arc::new(BinanceSigner::new(
        "test_api_key".to_string(),
        "test_api_secret".to_string(),
    ));
    let ws = UserDataWs::new_perpetual(
        "test_listen_key".to_string(),
        client,
        signer,
    );
    assert_eq!(
        ws.ws_url,
        "wss://fstream.binance.com/private/ws?listenKey=test_listen_key"
    );
}


fn make_test_inner(
    status: &str,
    orig_qty: &str,
    filled_qty: &str,
    remaining_qty: &str,
    last_fill_price: &str,
    last_fill_qty: &str,
    commission: &str,
) -> BinanceOrderInner {
    BinanceOrderInner {
        symbol: "BTCUSDT".to_string(),
        client_order_id: "test_client".to_string(),
        side: "BUY".to_string(),
        order_type: "LIMIT".to_string(),
        status: status.to_string(),
        order_id: 123456789,
        orig_qty: orig_qty.to_string(),
        filled_qty: filled_qty.to_string(),
        remaining_qty: Some(remaining_qty.to_string()),
        last_fill_price: last_fill_price.to_string(),
        avg_fill_price: None,
        last_fill_qty: last_fill_qty.to_string(),
        commission: commission.to_string(),
        commission_asset: "USDT".to_string(),
        trade_time: 1713900000123,
        is_reduce_only: false,
        working_type: "CONTRACT_PRICE".to_string(),
        position_side: None,
    }
}
