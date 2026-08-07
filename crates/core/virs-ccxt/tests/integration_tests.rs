use serde_json::json;

use virs_ccxt::{
    BinanceExchange, dispatch_event, hmac_sha256_hex,
    create_exchange, parse_f64, parse_str,
};
use virs_error::ExchangeError;

use virs_type::{OrderStatus, CcxtOrderStatus};

#[test]
fn int_2_1_hmac_signature_deterministic() {
    let key = "test_secret_key";
    let msg = "symbol=BTCUSDT&timestamp=1234567890";
    let sig1 = hmac_sha256_hex(key, msg);
    let sig2 = hmac_sha256_hex(key, msg);
    assert_eq!(sig1, sig2);
    assert_eq!(sig1.len(), 64);
}

#[test]
fn int_4_2_order_trade_update_dispatch() {
    let raw = json!({
        "e": "ORDER_TRADE_UPDATE",
        "E": 1591276258743i64,
        "T": 1591276258732i64,
        "o": {
            "s": "BTCUSDT",
            "c": "test_order",
            "S": "BUY",
            "o": "LIMIT",
            "f": "GTC",
            "q": "0.001",
            "p": "50000",
            "ap": "50000",
            "x": "TRADE",
            "X": "FILLED",
            "i": 123456,
            "l": "0.001",
            "z": "0.001",
            "L": "50000",
            "n": "0.015",
            "N": "USDT",
            "T": 1591276258732i64,
            "t": 1,
            "m": false,
            "R": false,
            "w": "MARK_PRICE",
            "ps": "LONG"
        }
    });
    let text = serde_json::to_string(&raw).unwrap();
    let event = dispatch_event(&text);
    assert!(event.is_some());
}

#[test]
fn int_4_3_non_order_event_returns_none() {
    let raw = json!({"e": "listenKeyExpired", "E": 1234567890});
    let text = serde_json::to_string(&raw).unwrap();
    let event = dispatch_event(&text);
    assert!(event.is_none());
}

#[tokio::test]
async fn int_5_1_create_exchange_binance_hmac() {
    let result = create_exchange(
        "binance",
        "test_api_key",
        "test_api_secret",
        None,
        None,
        std::time::Duration::from_secs(30),
        std::time::Duration::from_secs(10),
        10,
        900,
    )
    .await;
    assert!(result.is_ok());
    let exchange = result.unwrap();
    assert_eq!(exchange.name(), "binance");
}

#[tokio::test]
async fn int_5_2_create_exchange_binance_ed25519() {
    let seed_b64 = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
    let result = create_exchange(
        "binance",
        "test_api_key",
        seed_b64,
        None,
        None,
        std::time::Duration::from_secs(30),
        std::time::Duration::from_secs(10),
        10,
        900,
    )
    .await;
    assert!(result.is_ok());
    let exchange = result.unwrap();
    assert_eq!(exchange.name(), "binance");
}

#[tokio::test]
async fn int_5_3_create_exchange_bybit_not_supported() {
    let result = create_exchange(
        "bybit",
        "key",
        "secret",
        None,
        None,
        std::time::Duration::from_secs(30),
        std::time::Duration::from_secs(10),
        10,
        900,
    )
    .await;
    assert!(result.is_err());
    match result.err().unwrap() {
        ExchangeError::NotSupported(_) => {}
        _ => panic!("Expected NotSupported error"),
    }
}

#[tokio::test]
async fn int_5_4_create_exchange_okx_not_supported() {
    let result = create_exchange(
        "okx",
        "key",
        "secret",
        None,
        None,
        std::time::Duration::from_secs(30),
        std::time::Duration::from_secs(10),
        10,
        900,
    )
    .await;
    assert!(result.is_err());
    match result.err().unwrap() {
        ExchangeError::NotSupported(_) => {}
        _ => panic!("Expected NotSupported error"),
    }
}

#[tokio::test]
async fn int_5_5_create_exchange_case_insensitive() {
    let result = create_exchange(
        "BINANCE",
        "key",
        "secret",
        None,
        None,
        std::time::Duration::from_secs(30),
        std::time::Duration::from_secs(10),
        10,
        900,
    )
    .await;
    assert!(result.is_ok());
}

#[test]
fn int_6_2_order_status_chain() {
    let binance_status = "PARTIALLY_FILLED";
    let ccxt_status = BinanceExchange::parse_order_status(binance_status);
    assert_eq!(ccxt_status, CcxtOrderStatus::PartiallyFilled);

    let app_status: OrderStatus = ccxt_status.into();
    assert_eq!(app_status, OrderStatus::PartiallyFilled);
}

#[test]
fn int_6_3_order_status_expired_chain() {
    let binance_status = "EXPIRED";
    let ccxt_status = BinanceExchange::parse_order_status(binance_status);
    assert_eq!(ccxt_status, CcxtOrderStatus::Expired);

    let app_status: OrderStatus = ccxt_status.into();
    assert_eq!(app_status, OrderStatus::Expired);
}

#[test]
fn int_7_1_order_type_roundtrip() {
    use virs_type::OrderType;

    let types = vec![
        OrderType::Market,
        OrderType::Limit,
        OrderType::Stop,
        OrderType::StopMarket,
        OrderType::TakeProfit,
        OrderType::TakeProfitMarket,
        OrderType::TrailingStopMarket,
    ];

    for ot in &types {
        let binance_str = BinanceExchange::order_type_str(ot);
        let parsed = BinanceExchange::parse_order_type(&binance_str);
        assert_eq!(&parsed, ot, "Roundtrip failed for {:?}", ot);
    }
}

#[test]
fn int_7_2_side_roundtrip() {
    use virs_type::Side;

    assert_eq!(BinanceExchange::side_str(&Side::Buy), "BUY");
    assert_eq!(BinanceExchange::side_str(&Side::Sell), "SELL");
}

#[test]
fn int_8_1_parse_f64_used_in_ticker_conversion() {
    let raw = json!({"price": "0.00012345"});
    let val = parse_f64(&raw, "price");
    assert_eq!(val, Some(0.00012345));
}

#[test]
fn int_8_3_parse_str_used_in_symbol() {
    let raw = json!({"symbol": "BTCUSDT"});
    let val = parse_str(&raw, "symbol");
    assert_eq!(val, Some("BTCUSDT".to_string()));
}
