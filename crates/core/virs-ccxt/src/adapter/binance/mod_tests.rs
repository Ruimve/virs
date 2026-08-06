use crate::adapter::binance::{
    try_build_ed25519, BinanceExchange, TIME_OFFSET_WARN_THRESHOLD_MS,
    TIME_SYNC_INTERVAL_SECS,
};
use virs_type::{CcxtOrderStatus, OrderType, Side};

#[test]
fn b1_1_native_symbol_with_slash() {
    assert_eq!(BinanceExchange::to_native_symbol("BTC/USDT"), "BTCUSDT");
}

#[test]
fn b1_2_native_symbol_with_dash() {
    assert_eq!(BinanceExchange::to_native_symbol("BTC-USDT"), "BTCUSDT");
}

#[test]
fn b1_3_native_symbol_already_native() {
    assert_eq!(BinanceExchange::to_native_symbol("BTCUSDT"), "BTCUSDT");
}

#[test]
fn b1_4_native_symbol_eth_usdc() {
    assert_eq!(BinanceExchange::to_native_symbol("ETH/USDC"), "ETHUSDC");
}

#[test]
fn b1_5_native_symbol_empty() {
    assert_eq!(BinanceExchange::to_native_symbol(""), "");
}

#[test]
fn b2_1_unified_symbol_usdt() {
    assert_eq!(BinanceExchange::to_unified_symbol("BTCUSDT"), "BTC/USDT");
}

#[test]
fn b2_2_unified_symbol_usdc() {
    assert_eq!(BinanceExchange::to_unified_symbol("ETHUSDC"), "ETH/USDC");
}

#[test]
fn b2_3_unified_symbol_btc_pair() {
    assert_eq!(BinanceExchange::to_unified_symbol("BNBBTC"), "BNB/BTC");
}

#[test]
fn b2_4_unified_symbol_busd() {
    assert_eq!(BinanceExchange::to_unified_symbol("BTCBUSD"), "BTC/BUSD");
}

#[test]
fn b2_5_unified_symbol_unknown_quote() {
    assert_eq!(BinanceExchange::to_unified_symbol("BTCXYZ"), "BTCXYZ");
}

#[test]
fn b2_6_unified_symbol_only_quote() {
    assert_eq!(BinanceExchange::to_unified_symbol("USDT"), "USDT");
}

#[test]
fn b3_1_status_new() {
    assert_eq!(
        BinanceExchange::parse_order_status("NEW"),
        CcxtOrderStatus::New
    );
}

#[test]
fn b3_2_status_partially_filled() {
    assert_eq!(
        BinanceExchange::parse_order_status("PARTIALLY_FILLED"),
        CcxtOrderStatus::PartiallyFilled
    );
}

#[test]
fn b3_3_status_filled() {
    assert_eq!(
        BinanceExchange::parse_order_status("FILLED"),
        CcxtOrderStatus::Filled
    );
}

#[test]
fn b3_4_status_canceled() {
    assert_eq!(
        BinanceExchange::parse_order_status("CANCELED"),
        CcxtOrderStatus::Canceled
    );
}

#[test]
fn b3_5_status_cancelled_variant() {
    assert_eq!(
        BinanceExchange::parse_order_status("CANCELLED"),
        CcxtOrderStatus::Canceled
    );
}

#[test]
fn b3_6_status_expired() {
    assert_eq!(
        BinanceExchange::parse_order_status("EXPIRED"),
        CcxtOrderStatus::Expired
    );
}

#[test]
fn b3_7_status_expired_in_match() {
    assert_eq!(
        BinanceExchange::parse_order_status("EXPIRED_IN_MATCH"),
        CcxtOrderStatus::ExpiredInMatch
    );
}

#[test]
fn b3_8_status_unknown_returns_unknown() {
    assert_eq!(
        BinanceExchange::parse_order_status("UNKNOWN"),
        CcxtOrderStatus::Unknown("UNKNOWN".to_string())
    );
}

#[test]
fn b4_1_type_market() {
    assert_eq!(
        BinanceExchange::parse_order_type("MARKET"),
        OrderType::Market
    );
}

#[test]
fn b4_2_type_limit() {
    assert_eq!(BinanceExchange::parse_order_type("LIMIT"), OrderType::Limit);
}

#[test]
fn b4_3_type_stop_market() {
    assert_eq!(
        BinanceExchange::parse_order_type("STOP_MARKET"),
        OrderType::StopMarket
    );
}

#[test]
fn b4_4_type_stop() {
    assert_eq!(BinanceExchange::parse_order_type("STOP"), OrderType::Stop);
}

#[test]
fn b4_5_type_trailing_stop_market() {
    assert_eq!(
        BinanceExchange::parse_order_type("TRAILING_STOP_MARKET"),
        OrderType::TrailingStopMarket
    );
}

#[test]
fn b4_6_type_liquidation() {
    assert_eq!(
        BinanceExchange::parse_order_type("LIQUIDATION"),
        OrderType::Liquidation
    );
}

#[test]
fn b4_7_type_take_profit_market() {
    assert_eq!(
        BinanceExchange::parse_order_type("TAKE_PROFIT_MARKET"),
        OrderType::TakeProfitMarket
    );
}

#[test]
fn b4_7b_type_take_profit() {
    assert_eq!(
        BinanceExchange::parse_order_type("TAKE_PROFIT"),
        OrderType::TakeProfit
    );
}

#[test]
fn b4_8_type_unknown_returns_unknown() {
    assert_eq!(
        BinanceExchange::parse_order_type("UNKNOWN"),
        OrderType::Unknown("UNKNOWN".to_string())
    );
}

#[test]
fn b5_1_side_buy() {
    assert_eq!(BinanceExchange::side_str(&Side::Buy), "BUY");
}

#[test]
fn b5_2_side_sell() {
    assert_eq!(BinanceExchange::side_str(&Side::Sell), "SELL");
}

#[test]
fn b6_1_order_type_market() {
    assert_eq!(
        BinanceExchange::order_type_str(&OrderType::Market),
        "MARKET"
    );
}

#[test]
fn b6_2_order_type_limit() {
    assert_eq!(BinanceExchange::order_type_str(&OrderType::Limit), "LIMIT");
}

#[test]
fn b6_3_order_type_stop_market() {
    assert_eq!(
        BinanceExchange::order_type_str(&OrderType::StopMarket),
        "STOP_MARKET"
    );
}

#[test]
fn b6_4_order_type_stop() {
    assert_eq!(BinanceExchange::order_type_str(&OrderType::Stop), "STOP");
}

#[test]
fn b6_5_order_type_take_profit_market() {
    assert_eq!(
        BinanceExchange::order_type_str(&OrderType::TakeProfitMarket),
        "TAKE_PROFIT_MARKET"
    );
}

#[test]
fn b6_6_futures_stop_market_unchanged() {
    assert_eq!(
        BinanceExchange::order_type_str(&OrderType::StopMarket),
        "STOP_MARKET"
    );
}

#[test]
fn b6_7_futures_take_profit_market_unchanged() {
    assert_eq!(
        BinanceExchange::order_type_str(&OrderType::TakeProfitMarket),
        "TAKE_PROFIT_MARKET"
    );
}

#[test]
fn b6_8_order_type_take_profit() {
    assert_eq!(
        BinanceExchange::order_type_str(&OrderType::TakeProfit),
        "TAKE_PROFIT"
    );
}

#[test]
fn b6_9_order_type_trailing_stop_market() {
    assert_eq!(
        BinanceExchange::order_type_str(&OrderType::TrailingStopMarket),
        "TRAILING_STOP_MARKET"
    );
}

#[test]
fn b6_10_order_type_liquidation() {
    assert_eq!(
        BinanceExchange::order_type_str(&OrderType::Liquidation),
        "LIQUIDATION"
    );
}

#[test]
fn b7_1_try_build_ed25519_with_seed() {
    let seed_b64 = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
    let result = try_build_ed25519("test_api_key", seed_b64);
    assert!(result.is_ok());
}

#[test]
fn b7_2_try_build_ed25519_with_pem() {
    let pem = "-----BEGIN PRIVATE KEY-----\n\
               MC4CAQAwBQYDK2VwBCIEIHTrQ7Yvl4pKl3jY6DLv0DqgjFLf7tAfFGD7T0rJ1y3J\n\
               -----END PRIVATE KEY-----";
    let result = try_build_ed25519("test_key", pem);
    assert!(result.is_ok());
}

#[test]
fn b7_3_try_build_ed25519_wrong_byte_count() {
    let wrong_b64 = "AAAAAAAAAAAAAAAAAAAAAA==";
    let result = try_build_ed25519("key", wrong_b64);
    assert!(result.is_err());
}

#[test]
fn b7_4_try_build_ed25519_not_base64() {
    let result = try_build_ed25519("key", "this_is_not_base64_or_pem!");
    assert!(result.is_err());
}

#[test]
fn t1_1_time_sync_interval_is_one_hour() {
    assert_eq!(TIME_SYNC_INTERVAL_SECS, 3600);
}

#[test]
fn t1_2_time_offset_warn_threshold_is_2000ms() {
    assert_eq!(TIME_OFFSET_WARN_THRESHOLD_MS, 2_000);
}

#[test]
fn t1_3_time_sync_started_initialized_false() {
    let ex = BinanceExchange::new(
        "test_key",
        "test_secret",
        None,
        std::time::Duration::from_secs(30),
        std::time::Duration::from_secs(10),
        10,
        900,
    )
    .unwrap();

    assert!(!ex
        .time_sync_started
        .load(std::sync::atomic::Ordering::SeqCst));
}

#[test]
fn t1_4_time_sync_started_swap_prevents_double_start() {
    let ex = BinanceExchange::new(
        "test_key",
        "test_secret",
        None,
        std::time::Duration::from_secs(30),
        std::time::Duration::from_secs(10),
        10,
        900,
    )
    .unwrap();

    let first = ex
        .time_sync_started
        .swap(true, std::sync::atomic::Ordering::SeqCst);
    assert!(!first, "first swap should return false (not yet started)");

    let second = ex
        .time_sync_started
        .swap(true, std::sync::atomic::Ordering::SeqCst);
    assert!(second, "second swap should return true (already started)");
}

#[test]
fn t1_5_drop_cancels_time_sync() {
    let ex = BinanceExchange::new(
        "test_key",
        "test_secret",
        None,
        std::time::Duration::from_secs(30),
        std::time::Duration::from_secs(10),
        10,
        900,
    )
    .unwrap();

    drop(ex);
}

#[tokio::test]
async fn t1_6_no_tasks_on_init() {
    let ex = BinanceExchange::new(
        "test_key",
        "test_secret",
        None,
        std::time::Duration::from_secs(30),
        std::time::Duration::from_secs(10),
        10,
        900,
    )
    .unwrap();

    assert!(
        ex.time_sync_task.lock().unwrap().is_none(),
        "time_sync_task should be None on init (before sync_time)"
    );
    assert!(
        ex.listenkey_task.lock().unwrap().is_none(),
        "listenkey_task should be None on init"
    );
}
