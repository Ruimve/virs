use serde_json::json;

use virs_ccxt::{
    adapter::binance::{user_data_ws::BinanceOrderMessage, BinanceExchange},
    auth::hmac_sha256_hex,
    create_exchange, parse_f64, parse_str,
    types::{CcxtOrderStatus, CcxtTicker},
};
use virs_error::ExchangeError;

use virs_types::enums::OrderStatus;


#[test]
fn int_1_1_symbol_roundtrip_usdt() {
    let native = BinanceExchange::to_native_symbol("BTC/USDT");
    assert_eq!(native, "BTCUSDT");
    let unified = BinanceExchange::to_unified_symbol(&native);
    assert_eq!(unified, "BTC/USDT");
}

#[test]
fn int_1_2_symbol_roundtrip_usdc() {
    let native = BinanceExchange::to_native_symbol("ETH-USDC");
    assert_eq!(native, "ETHUSDC");
    let unified = BinanceExchange::to_unified_symbol(&native);
    assert_eq!(unified, "ETH/USDC");
}

#[test]
fn int_1_3_symbol_roundtrip_btc_pair() {
    let native = BinanceExchange::to_native_symbol("BNB/BTC");
    assert_eq!(native, "BNBBTC");
    let unified = BinanceExchange::to_unified_symbol(&native);
    assert_eq!(unified, "BNB/BTC");
}


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
fn int_4_2_order_trade_update_to_ws_feed_event() {
    let raw = json!({
        "e": "ORDER_TRADE_UPDATE",
        "E": 1591276258743i64,
        "T": 1591276258732i64,
        "o": {
            "s": "BTCUSDT",
            "c": "test_order",
            "S": "BUY",
            "o": "LIMIT",
            "X": "FILLED",
            "i": 123456,
            "q": "0.001",
            "z": "0.001",
            "L": "50000",
            "l": "0.001",
            "n": "0.015",
            "N": "USDT",
            "T": 1591276258732i64,
            "R": false,
            "w": "MARK_PRICE"
        }
    });
    let msg: BinanceOrderMessage = serde_json::from_value(raw).unwrap();
    let event = msg.to_ws_feed_event();
    assert!(event.is_some());
}

#[test]
fn int_4_3_non_order_event_returns_none() {
    let raw = json!({"e": "listenKeyExpired", "E": 1234567890});
    let msg: BinanceOrderMessage = serde_json::from_value(raw).unwrap();
    let event = msg.to_ws_feed_event();
    assert_eq!(event, None);
}


#[test]
fn int_5_1_create_exchange_binance_hmac() {
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
    );
    assert!(result.is_ok());
    let exchange = result.unwrap();
    assert_eq!(exchange.id(), "binance");
    assert_eq!(exchange.name(), "Binance");
}

#[test]
fn int_5_2_create_exchange_binance_ed25519() {

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
    );
    assert!(result.is_ok());
    let exchange = result.unwrap();
    assert_eq!(exchange.id(), "binance");
}

#[test]
fn int_5_3_create_exchange_bybit_not_supported() {
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
    );
    assert!(result.is_err());
    match result.err().unwrap() {
        ExchangeError::NotSupported(_) => {}
        _ => panic!("Expected NotSupported error"),
    }
}

#[test]
fn int_5_4_create_exchange_okx_not_supported() {
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
    );
    assert!(result.is_err());
    match result.err().unwrap() {
        ExchangeError::NotSupported(_) => {}
        _ => panic!("Expected NotSupported error"),
    }
}

#[test]
fn int_5_5_create_exchange_case_insensitive() {
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
    );
    assert!(result.is_ok());
}


#[test]
fn int_6_1_ticker_json_to_ticker_via_parse() {

    let raw = json!({
        "symbol": "BTCUSDT",
        "bidPrice": "50000.0",
        "askPrice": "50001.0",
        "lastPrice": "50000.5",
        "highPrice": "51000.0",
        "lowPrice": "49000.0",
        "volume": "1000.5",
        "priceChange": "500.5",
        "priceChangePercent": "1.01",
    });

    let ccxt = CcxtTicker {
        symbol: "BTC/USDT".into(),
        exchange: "binance".into(),
        bid: parse_f64(&raw, "bidPrice"),
        ask: parse_f64(&raw, "askPrice"),
        last: parse_f64(&raw, "lastPrice"),
        high: parse_f64(&raw, "highPrice"),
        low: parse_f64(&raw, "lowPrice"),
        volume: parse_f64(&raw, "volume"),
        quote_volume: None,
        open: None,
        close: None,
        price_change: parse_f64(&raw, "priceChange"),
        price_change_pct: parse_f64(&raw, "priceChangePercent"),
        timestamp: None,
        info: raw,
    };

    let ticker: virs_types::market::Ticker = ccxt.try_into().unwrap();
    assert_eq!(ticker.symbol, "BTC/USDT");
    assert_eq!(ticker.exchange, "binance");
    assert_eq!(ticker.bid, Some(50000.0));
    assert_eq!(ticker.ask, Some(50001.0));
    assert_eq!(ticker.last, 50000.5);
    assert_eq!(ticker.high_24h, 51000.0);
    assert_eq!(ticker.low_24h, 49000.0);
    assert_eq!(ticker.volume_24h, 1000.5);
    assert_eq!(ticker.price_change_24h, 500.5);
    assert!((ticker.price_change_pct_24h - 1.01).abs() < 0.001);
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
fn int_6_3_order_status_expired_to_canceled_chain() {
    let binance_status = "EXPIRED";
    let ccxt_status = BinanceExchange::parse_order_status(binance_status);
    assert_eq!(ccxt_status, CcxtOrderStatus::Canceled);

    let app_status: OrderStatus = ccxt_status.into();
    assert_eq!(app_status, OrderStatus::Canceled);
}


#[test]
fn int_7_1_order_type_roundtrip() {
    use virs_ccxt::types::OrderType;

    let types = vec![
        OrderType::Market,
        OrderType::Limit,
        OrderType::StopMarket,
        OrderType::StopLimit,
        OrderType::TakeProfitMarket,
    ];

    for ot in &types {
        let binance_str = BinanceExchange::order_type_str(ot);
        let parsed = BinanceExchange::parse_order_type(binance_str);
        assert_eq!(&parsed, ot, "Roundtrip failed for {:?}", ot);
    }
}

#[test]
fn int_7_2_side_roundtrip() {
    use virs_ccxt::types::Side;

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
