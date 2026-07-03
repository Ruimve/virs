//! Unit tests for adapter/binance/mod.rs mapping functions.
//!
//! Covers: to_native_symbol, to_unified_symbol, parse_order_status,
//! parse_order_type, side_str, order_type_str, try_build_ed25519,
//! parse_order_book_side (shared).

use serde_json::json;

use crate::adapter::binance::{parse_order_book_side, try_build_ed25519, BinanceExchange};
use crate::types::{CcxtOrderStatus, OrderType, Side};

// ============================================================
// TC-B1: to_native_symbol
// ============================================================

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

// ============================================================
// TC-B2: to_unified_symbol
// ============================================================

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
    // "USDT" → base is empty → returns as-is
    assert_eq!(BinanceExchange::to_unified_symbol("USDT"), "USDT");
}

// ============================================================
// TC-B3: parse_order_status
// ============================================================

#[test]
fn b3_1_status_new() {
    assert_eq!(
        BinanceExchange::parse_order_status("NEW"),
        CcxtOrderStatus::Open
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
        CcxtOrderStatus::Canceled
    );
}

#[test]
fn b3_7_status_rejected() {
    assert_eq!(
        BinanceExchange::parse_order_status("REJECTED"),
        CcxtOrderStatus::Rejected
    );
}

#[test]
fn b3_8_status_pending_cancel() {
    assert_eq!(
        BinanceExchange::parse_order_status("PENDING_CANCEL"),
        CcxtOrderStatus::Open
    );
}

#[test]
fn b3_9_status_unknown_defaults_to_open() {
    assert_eq!(
        BinanceExchange::parse_order_status("UNKNOWN"),
        CcxtOrderStatus::Open
    );
}

// ============================================================
// TC-B4: parse_order_type
// ============================================================

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
fn b4_4_type_stop_loss() {
    assert_eq!(
        BinanceExchange::parse_order_type("STOP_LOSS"),
        OrderType::StopMarket
    );
}

#[test]
fn b4_5_type_stop_loss_limit() {
    assert_eq!(
        BinanceExchange::parse_order_type("STOP_LOSS_LIMIT"),
        OrderType::StopLimit
    );
}

#[test]
fn b4_6_type_take_profit_limit() {
    assert_eq!(
        BinanceExchange::parse_order_type("TAKE_PROFIT_LIMIT"),
        OrderType::StopLimit
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
fn b4_8_type_unknown_defaults_to_market() {
    assert_eq!(
        BinanceExchange::parse_order_type("UNKNOWN"),
        OrderType::Market
    );
}

// ============================================================
// TC-B5: side_str
// ============================================================

#[test]
fn b5_1_side_buy() {
    assert_eq!(BinanceExchange::side_str(&Side::Buy), "BUY");
}

#[test]
fn b5_2_side_sell() {
    assert_eq!(BinanceExchange::side_str(&Side::Sell), "SELL");
}

// ============================================================
// TC-B6: order_type_str
// ============================================================

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
fn b6_4_order_type_stop_limit() {
    // 现货 StopLimit → STOP_LOSS_LIMIT
    assert_eq!(
        BinanceExchange::order_type_str(&OrderType::StopLimit),
        "STOP_LOSS_LIMIT"
    );
    // 合约 StopLimit → STOP
    assert_eq!(
        BinanceExchange::order_type_str_futures(&OrderType::StopLimit),
        "STOP"
    );
}

#[test]
fn b6_5_order_type_take_profit_market() {
    assert_eq!(
        BinanceExchange::order_type_str(&OrderType::TakeProfitMarket),
        "TAKE_PROFIT_MARKET"
    );
}

// ============================================================
// TC-B7: try_build_ed25519
// ============================================================

#[test]
fn b7_1_try_build_ed25519_with_seed() {
    // A valid 32-byte base64 seed
    // "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=" is 32 zero bytes in base64
    let seed_b64 = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
    let result = try_build_ed25519("test_api_key", seed_b64);
    assert!(result.is_ok());
    let signer = result.unwrap();
    assert_eq!(signer.api_key(), "test_api_key");
}

#[test]
fn b7_2_try_build_ed25519_with_pem() {
    // Generate a minimal valid PKCS8 PEM key for ed25519
    // This is a well-known test key
    let pem = "-----BEGIN PRIVATE KEY-----\n\
               MC4CAQAwBQYDK2VwBCIEIHTrQ7Yvl4pKl3jY6DLv0DqgjFLf7tAfFGD7T0rJ1y3J\n\
               -----END PRIVATE KEY-----";
    let result = try_build_ed25519("test_key", pem);
    assert!(result.is_ok());
}

#[test]
fn b7_3_try_build_ed25519_wrong_byte_count() {
    // 16 bytes in base64 (not 32) → should fail
    let wrong_b64 = "AAAAAAAAAAAAAAAAAAAAAA==";
    let result = try_build_ed25519("key", wrong_b64);
    assert!(result.is_err());
}

#[test]
fn b7_4_try_build_ed25519_not_base64() {
    // Not valid base64, not PEM → should fail (HMAC fallback)
    let result = try_build_ed25519("key", "this_is_not_base64_or_pem!");
    assert!(result.is_err());
}

// ============================================================
// TC-F1: parse_order_book_side (shared function)
// ============================================================

#[test]
fn f1_1_parse_order_book_side_bids() {
    let data = json!({
        "bids": [["50000.0", "1.5"], ["49999.0", "2.0"]],
        "asks": [["50001.0", "1.0"]]
    });
    let bids = parse_order_book_side(&data, "bids");
    assert_eq!(bids, vec![(50000.0, 1.5), (49999.0, 2.0)]);
}

#[test]
fn f1_2_parse_order_book_side_asks() {
    let data = json!({
        "bids": [["50000.0", "1.5"]],
        "asks": [["50001.0", "1.0"], ["50002.0", "0.5"]]
    });
    let asks = parse_order_book_side(&data, "asks");
    assert_eq!(asks, vec![(50001.0, 1.0), (50002.0, 0.5)]);
}

#[test]
fn f1_3_parse_order_book_side_missing() {
    let data = json!({"bids": [["50000.0", "1.0"]]});
    let asks = parse_order_book_side(&data, "asks");
    assert!(asks.is_empty());
}

#[test]
fn f1_4_parse_order_book_side_empty() {
    let data = json!({"bids": [], "asks": []});
    let bids = parse_order_book_side(&data, "bids");
    assert!(bids.is_empty());
}
