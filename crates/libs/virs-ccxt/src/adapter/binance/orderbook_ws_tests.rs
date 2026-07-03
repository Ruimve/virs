//! Unit tests for adapter/binance/orderbook_ws.rs parsing functions.
//!
//! Covers: parse_levels, to_levels, parse_payload, BinanceDepthMessage::into_depth.

use serde_json::json;

use crate::adapter::binance::orderbook_ws::{
    parse_levels, parse_payload, to_levels, BinanceDepthMessage,
};
use crate::ws_types::OrderBookLevel;

// ============================================================
// TC-W1: parse_levels
// ============================================================

#[test]
fn w1_1_parse_levels_standard() {
    let v = json!([["50000.0", "1.5"], ["49999.0", "2.0"]]);
    let result = parse_levels(&v);
    assert_eq!(
        result,
        Some(vec![
            ["50000.0".to_string(), "1.5".to_string()],
            ["49999.0".to_string(), "2.0".to_string()],
        ])
    );
}

#[test]
fn w1_2_parse_levels_numeric_elements() {
    let v = json!([[50000.0, 1.5]]);
    let result = parse_levels(&v);
    assert!(result.is_some());
    let levels = result.unwrap();
    assert_eq!(levels.len(), 1);
    // Numbers should be converted to strings
    assert!((levels[0][0].parse::<f64>().unwrap() - 50000.0).abs() < 0.001);
}

#[test]
fn w1_3_parse_levels_empty_array() {
    let v = json!([]);
    let result = parse_levels(&v);
    assert_eq!(result, Some(vec![]));
}

#[test]
fn w1_4_parse_levels_not_array() {
    let v = json!({"key": "value"});
    let result = parse_levels(&v);
    assert_eq!(result, None);
}

#[test]
fn w1_5_parse_levels_short_element() {
    // Element with only 1 item → should return None
    let v = json!([["50000.0"]]);
    let result = parse_levels(&v);
    assert_eq!(result, None);
}

// ============================================================
// TC-W2: to_levels
// ============================================================

#[test]
fn w2_1_to_levels_normal() {
    let raw = [
        ["50000.0".to_string(), "1.5".to_string()],
        ["49999.0".to_string(), "2.0".to_string()],
    ];
    let levels = to_levels(&raw);
    assert_eq!(
        levels,
        vec![
            OrderBookLevel {
                price: 50000.0,
                amount: 1.5
            },
            OrderBookLevel {
                price: 49999.0,
                amount: 2.0
            },
        ]
    );
}

#[test]
fn w2_2_to_levels_filter_zero_amount() {
    let raw = [
        ["50000.0".to_string(), "0.0".to_string()],
        ["49999.0".to_string(), "2.0".to_string()],
    ];
    let levels = to_levels(&raw);
    assert_eq!(levels.len(), 1);
    assert_eq!(levels[0].price, 49999.0);
}

#[test]
fn w2_3_to_levels_filter_negative_amount() {
    let raw = [
        ["50000.0".to_string(), "-1.0".to_string()],
        ["49999.0".to_string(), "2.0".to_string()],
    ];
    let levels = to_levels(&raw);
    assert_eq!(levels.len(), 1);
    assert_eq!(levels[0].price, 49999.0);
}

#[test]
fn w2_4_to_levels_filter_invalid_number() {
    let raw = [
        ["abc".to_string(), "1.0".to_string()],
        ["49999.0".to_string(), "2.0".to_string()],
    ];
    let levels = to_levels(&raw);
    assert_eq!(levels.len(), 1);
    assert_eq!(levels[0].price, 49999.0);
}

#[test]
fn w2_5_to_levels_empty() {
    let raw: [[String; 2]; 0] = [];
    let levels = to_levels(&raw);
    assert!(levels.is_empty());
}

// ============================================================
// TC-W3: parse_payload
// ============================================================

#[test]
fn w3_1_parse_payload_spot_format() {
    let v = json!({
        "bids": [["50000.0", "1.5"]],
        "asks": [["50001.0", "1.0"]],
        "lastUpdateId": 160
    });
    let result = parse_payload(&v);
    assert!(result.is_some());
    let (bids, asks, sym, ts) = result.unwrap();
    assert_eq!(bids.len(), 1);
    assert_eq!(asks.len(), 1);
    assert!(sym.is_none());
    assert_eq!(ts, 160);
}

#[test]
fn w3_2_parse_payload_perpetual_format() {
    let v = json!({
        "e": "depthUpdate",
        "E": 1234567890,
        "T": 1234567890,
        "s": "BTCUSDT",
        "U": 157,
        "u": 160,
        "pu": 149,
        "b": [["50000.0", "1.5"]],
        "a": [["50001.0", "1.0"]]
    });
    let result = parse_payload(&v);
    assert!(result.is_some());
    let (bids, asks, sym, ts) = result.unwrap();
    assert_eq!(bids.len(), 1);
    assert_eq!(asks.len(), 1);
    assert_eq!(sym, Some("BTCUSDT".to_string()));
    assert_eq!(ts, 1234567890);
}

#[test]
fn w3_3_parse_payload_no_matching_format() {
    let v = json!({"foo": "bar"});
    let result = parse_payload(&v);
    assert_eq!(result, None);
}

// ============================================================
// TC-W4: BinanceDepthMessage::into_depth
// ============================================================

#[test]
fn w4_1_into_depth_combined_stream_spot() {
    let raw = json!({
        "stream": "btcusdt@depth20@500ms",
        "data": {
            "bids": [["50000.0", "1.5"]],
            "asks": [["50001.0", "1.0"]],
            "lastUpdateId": 160
        }
    });
    let msg: BinanceDepthMessage = serde_json::from_value(raw).unwrap();
    let result = msg.into_depth();
    assert!(result.is_some());
    let (bids, asks, stream, sym, ts) = result.unwrap();
    assert_eq!(bids.len(), 1);
    assert_eq!(asks.len(), 1);
    assert_eq!(stream, Some("btcusdt@depth20@500ms".to_string()));
    assert!(sym.is_none());
    assert_eq!(ts, 160);
}

#[test]
fn w4_2_into_depth_combined_stream_perpetual() {
    let raw = json!({
        "stream": "btcusdt@depth20@500ms",
        "data": {
            "e": "depthUpdate",
            "E": 1234567890,
            "T": 1234567890,
            "s": "BTCUSDT",
            "U": 157,
            "u": 160,
            "pu": 149,
            "b": [["50000.0", "1.5"]],
            "a": [["50001.0", "1.0"]]
        }
    });
    let msg: BinanceDepthMessage = serde_json::from_value(raw).unwrap();
    let result = msg.into_depth();
    assert!(result.is_some());
    let (bids, asks, stream, sym, ts) = result.unwrap();
    assert_eq!(bids.len(), 1);
    assert_eq!(asks.len(), 1);
    assert_eq!(stream, Some("btcusdt@depth20@500ms".to_string()));
    assert_eq!(sym, Some("BTCUSDT".to_string()));
    assert_eq!(ts, 1234567890);
}

#[test]
fn w4_3_into_depth_single_stream_spot() {
    let raw = json!({
        "bids": [["50000.0", "1.5"]],
        "asks": [["50001.0", "1.0"]],
        "lastUpdateId": 160
    });
    let msg: BinanceDepthMessage = serde_json::from_value(raw).unwrap();
    let result = msg.into_depth();
    assert!(result.is_some());
    let (bids, asks, _stream, sym, ts) = result.unwrap();
    assert_eq!(bids.len(), 1);
    assert_eq!(asks.len(), 1);
    assert!(sym.is_none());
    assert_eq!(ts, 160);
}

#[test]
fn w4_4_into_depth_single_stream_perpetual() {
    let raw = json!({
        "e": "depthUpdate",
        "E": 1234567890,
        "T": 1234567890,
        "s": "BTCUSDT",
        "U": 157,
        "u": 160,
        "pu": 149,
        "b": [["50000.0", "1.5"]],
        "a": [["50001.0", "1.0"]]
    });
    let msg: BinanceDepthMessage = serde_json::from_value(raw).unwrap();
    let result = msg.into_depth();
    assert!(result.is_some());
    let (bids, asks, _stream, sym, ts) = result.unwrap();
    assert_eq!(bids.len(), 1);
    assert_eq!(asks.len(), 1);
    assert_eq!(sym, Some("BTCUSDT".to_string()));
    assert_eq!(ts, 1234567890);
}

#[test]
fn w4_5_into_depth_invalid_message() {
    // A message with neither data nor bids/asks nor b/a
    let raw = json!({"foo": "bar"});
    let msg: BinanceDepthMessage = serde_json::from_value(raw).unwrap();
    let result = msg.into_depth();
    assert_eq!(result, None);
}
