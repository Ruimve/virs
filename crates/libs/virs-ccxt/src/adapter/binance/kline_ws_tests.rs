use crate::adapter::binance::kline_ws::{
    binance_ws_symbol, BinanceKlineData, BinanceKlineInner, BinanceKlineMessage, KlineWs,
    KLINE_WS_DELAY_THRESHOLD_MS,
};
use crate::ws_types::KlineWsClient;

// ========== 消息解析（5个） ==========

#[test]
fn test_parse_binance_kline_message() {
    let json = r#"{
        "stream": "btcusdt@kline_1m",
        "data": {
            "e": "kline",
            "E": 1713900000,
            "s": "BTCUSDT",
            "k": {
                "t": 1713900000000,
                "T": 1713900059999,
                "s": "BTCUSDT",
                "i": "1m",
                "o": "65000.00",
                "h": "65100.00",
                "l": "64900.00",
                "c": "65050.00",
                "v": "100.5",
                "n": 500,
                "x": false,
                "q": "6532500.00"
            }
        }
    }"#;

    let msg: BinanceKlineMessage = serde_json::from_str(json).unwrap();
    assert_eq!(msg.stream.as_deref(), Some("btcusdt@kline_1m"));
    assert!(msg.data.is_some());

    let data = msg.data.unwrap();
    assert_eq!(data.event_type, "kline");
    assert_eq!(data.kline.start_time, 1713900000000);
    assert_eq!(data.kline.end_time, 1713900059999);
    assert_eq!(data.kline.open, "65000.00");
    assert_eq!(data.kline.high, "65100.00");
    assert_eq!(data.kline.low, "64900.00");
    assert_eq!(data.kline.close, "65050.00");
    assert_eq!(data.kline.volume, "100.5");
    assert_eq!(data.kline.trades, 500);
    assert!(!data.kline.closed);
    assert_eq!(data.kline.quote_volume, "6532500.00");

    // Closed kline assertion (merged from test_parse_binance_kline_closed)
    let json_closed = r#"{
        "stream": "btcusdt@kline_1m",
        "data": {
            "e": "kline",
            "E": 1713900000,
            "s": "BTCUSDT",
            "k": {
                "t": 1713900000000,
                "T": 1713900059999,
                "s": "BTCUSDT",
                "i": "1m",
                "o": "65000.00",
                "h": "65100.00",
                "l": "64900.00",
                "c": "65050.00",
                "v": "100.5",
                "n": 500,
                "x": true,
                "q": "6532500.00"
            }
        }
    }"#;
    let msg_closed: BinanceKlineMessage = serde_json::from_str(json_closed).unwrap();
    let data_closed = msg_closed.data.unwrap();
    assert!(data_closed.kline.closed);

    // 单流格式（无 stream/data 包装）
    let json_flat = r#"{
        "e": "kline",
        "E": 1713900000,
        "s": "BTCUSDT",
        "k": {
            "t": 1713900000000,
            "T": 1713900059999,
            "s": "BTCUSDT",
            "i": "1m",
            "o": "65000.00",
            "h": "65100.00",
            "l": "64900.00",
            "c": "65050.00",
            "v": "100.5",
            "n": 500,
            "x": true,
            "q": "6532500.00"
        }
    }"#;
    let msg_flat: BinanceKlineMessage = serde_json::from_str(json_flat).unwrap();
    assert!(msg_flat.stream.is_none());
    assert!(msg_flat.data.is_none());
    assert_eq!(msg_flat.event_type_flat.as_deref(), Some("kline"));
    let data_flat = msg_flat.into_kline_data().unwrap();
    assert_eq!(data_flat.event_type, "kline");
    assert_eq!(data_flat.kline.start_time, 1713900000000);
    assert!(data_flat.kline.closed);
}

#[test]
fn test_parse_binance_kline_message_without_stream() {
    let json = r#"{
        "data": {
            "e": "kline",
            "E": 1713900000,
            "s": "BTCUSDT",
            "k": {
                "t": 1713900000000,
                "T": 1713900059999,
                "s": "BTCUSDT",
                "i": "1m",
                "o": "65000.00",
                "h": "65100.00",
                "l": "64900.00",
                "c": "65050.00",
                "v": "100.5",
                "n": 500,
                "x": false,
                "q": "6532500.00"
            }
        }
    }"#;

    let msg: BinanceKlineMessage = serde_json::from_str(json).unwrap();
    assert!(msg.stream.is_none());
    assert!(msg.data.is_some());
}

#[test]
fn test_parse_invalid_json() {
    let result: Result<BinanceKlineMessage, _> = serde_json::from_str("not json");
    assert!(result.is_err());
}

#[test]
fn test_parse_non_kline_event() {
    let json = r#"{
        "stream": "btcusdt@trade",
        "data": {
            "e": "trade",
            "E": 1713900000,
            "s": "BTCUSDT",
            "t": 12345,
            "p": "65000.00",
            "q": "1.5",
            "b": 100,
            "a": 200,
            "T": 1713900000123,
            "m": true,
            "M": true
        }
    }"#;

    // trade 事件没有 "k" 字段，反序列化应该失败
    let result: Result<BinanceKlineMessage, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

// ========== Candle 转换（4个） ==========

#[test]
fn test_to_candle_basic() {
    let data = BinanceKlineData {
        event_type: "kline".to_string(),
        event_time: 1713900000,
        kline: BinanceKlineInner {
            start_time: 1713900000000,
            end_time: 1713900059999,
            symbol: "BTCUSDT".to_string(),
            interval: "1m".to_string(),
            open: "65000.00".to_string(),
            high: "65100.00".to_string(),
            low: "64900.00".to_string(),
            close: "65050.00".to_string(),
            volume: "100.5".to_string(),
            trades: 500,
            closed: false,
            quote_volume: "6532500.00".to_string(),
        },
    };

    let candle = data.to_candle().expect("valid candle");
    assert_eq!(candle.open_time, 1713900000000);
    assert_eq!(candle.close_time, 1713900059999);
    assert!((candle.open - 65000.0).abs() < f64::EPSILON);
    assert!((candle.high - 65100.0).abs() < f64::EPSILON);
    assert!((candle.low - 64900.0).abs() < f64::EPSILON);
    assert!((candle.close - 65050.0).abs() < f64::EPSILON);
    assert!((candle.volume - 100.5).abs() < f64::EPSILON);
    assert!((candle.quote_volume - 6532500.0).abs() < f64::EPSILON);
    assert_eq!(candle.trades, 500);
    assert!(!candle.closed);

    // Closed candle assertion (merged from test_to_candle_closed)
    let data_closed = BinanceKlineData {
        event_type: "kline".to_string(),
        event_time: 1713900000,
        kline: BinanceKlineInner {
            start_time: 1713900000000,
            end_time: 1713900059999,
            symbol: "BTCUSDT".to_string(),
            interval: "1m".to_string(),
            open: "65000.00".to_string(),
            high: "65100.00".to_string(),
            low: "64900.00".to_string(),
            close: "65050.00".to_string(),
            volume: "100.5".to_string(),
            trades: 500,
            closed: true,
            quote_volume: "6532500.00".to_string(),
        },
    };
    let candle_closed = data_closed.to_candle().expect("valid closed candle");
    assert!(candle_closed.closed);
}

#[test]
fn test_to_candle_invalid_numbers() {
    let data = BinanceKlineData {
        event_type: "kline".to_string(),
        event_time: 1713900000,
        kline: BinanceKlineInner {
            start_time: 1713900000000,
            end_time: 1713900059999,
            symbol: "BTCUSDT".to_string(),
            interval: "1m".to_string(),
            open: "not_a_number".to_string(),
            high: "abc".to_string(),
            low: "64900.00".to_string(),
            close: "65050.00".to_string(),
            volume: "100.5".to_string(),
            trades: 500,
            closed: false,
            quote_volume: "6532500.00".to_string(),
        },
    };

    let result = data.to_candle();
    assert!(result.is_err(), "invalid OHLCV fields must return Err, not 0.0");
    let err = result.unwrap_err();
    assert!(
        matches!(err, virs_error::ExchangeError::NoData(_)),
        "expected NoData error, got {err:?}"
    );
}

#[test]
fn test_ws_symbol() {
    let data = BinanceKlineData {
        event_type: "kline".to_string(),
        event_time: 1713900000,
        kline: BinanceKlineInner {
            start_time: 1713900000000,
            end_time: 1713900059999,
            symbol: "BTCUSDT".to_string(),
            interval: "1m".to_string(),
            open: "65000.00".to_string(),
            high: "65100.00".to_string(),
            low: "64900.00".to_string(),
            close: "65050.00".to_string(),
            volume: "100.5".to_string(),
            trades: 500,
            closed: false,
            quote_volume: "6532500.00".to_string(),
        },
    };

    assert_eq!(data.ws_symbol(), "BTCUSDT");
}

// ========== Symbol 转换（3个） ==========

#[test]
fn test_binance_ws_symbol_basic() {
    assert_eq!(binance_ws_symbol("BTCUSDT"), "btcusdt");
    // With slash (merged from test_binance_ws_symbol_with_slash)
    assert_eq!(binance_ws_symbol("BTC/USDT"), "btcusdt");
    // Already lowercase (merged from test_binance_ws_symbol_lowercase)
    assert_eq!(binance_ws_symbol("btcusdt"), "btcusdt");
}

// ========== 构造函数和状态（2个） ==========

#[test]
fn test_new_perpetual() {
    let ws = KlineWs::new_perpetual(None, 1, 60, 30, 82800);
    assert_eq!(ws.ws_url, "wss://fstream.binance.com/market/ws");
    assert!(!ws.is_running());
}

#[tokio::test]
async fn test_subscribe_without_start() {
    let ws = KlineWs::new_perpetual(None, 1, 60, 30, 82800);
    assert!(!ws.is_running());

    // 不调用 start()，直接调用 subscribe
    ws.subscribe("BTCUSDT").await;

    // 验证 subscriptions 包含正确的 stream name
    let subs = ws.subscriptions.read().await;
    assert!(subs.contains(&"btcusdt@kline_1m".to_string()));

    // 验证 symbol_map 包含映射
    let map = ws.symbol_map.read().await;
    assert_eq!(map.get("btcusdt").unwrap(), "BTCUSDT");

    // 客户端仍然没有运行
    assert!(!ws.is_running());
}

// ========== T8: event_time 延迟检测（2个） ==========

#[test]
fn t8_1_event_time_parsed_and_accessible() {
    // T8: event_time (E field) must be parsed and accessible (no longer dead_code)
    let json = r#"{
        "stream": "btcusdt@kline_1m",
        "data": {
            "e": "kline",
            "E": 1713900000123,
            "s": "BTCUSDT",
            "k": {
                "t": 1713900000000,
                "T": 1713900059999,
                "s": "BTCUSDT",
                "i": "1m",
                "o": "65000.00",
                "h": "65100.00",
                "l": "64900.00",
                "c": "65050.00",
                "v": "100.5",
                "n": 500,
                "x": false,
                "q": "6532500.00"
            }
        }
    }"#;

    let msg: BinanceKlineMessage = serde_json::from_str(json).unwrap();
    let data = msg.into_kline_data().unwrap();
    // event_time must be parsed correctly
    assert_eq!(data.event_time, 1713900000123);
}

#[test]
fn t8_2_delay_threshold_is_5000ms() {
    // T8: The delay threshold constant must be 5000ms (5 seconds)
    assert_eq!(KLINE_WS_DELAY_THRESHOLD_MS, 5_000);

    // Verify delay calculation logic
    let event_time = 1713900000000_i64;
    let local_now = 1713900006000_i64; // 6 seconds later
    let delay_ms = local_now - event_time;
    assert!(delay_ms > KLINE_WS_DELAY_THRESHOLD_MS);

    // Below threshold
    let local_now_ok = 1713900003000_i64; // 3 seconds later
    let delay_ok = local_now_ok - event_time;
    assert!(delay_ok <= KLINE_WS_DELAY_THRESHOLD_MS);
}

// ============================================================
// T8 FAIL fix: 单流格式 event_time 解析
// ============================================================

#[test]
fn t8_3_single_stream_event_time_parsed() {
    // T8 FAIL fix: 单流格式（无 stream/data 包装）必须正确解析 E 字段
    // 此前 BinanceKlineMessage 缺少 #[serde(rename = "E")] 字段，event_time 恒为 0
    let json = r#"{
        "e": "kline",
        "E": 1713900000456,
        "s": "BTCUSDT",
        "k": {
            "t": 1713900000000,
            "T": 1713900059999,
            "s": "BTCUSDT",
            "i": "1m",
            "o": "65000.00",
            "h": "65100.00",
            "l": "64900.00",
            "c": "65050.00",
            "v": "100.5",
            "n": 500,
            "x": false,
            "q": "6532500.00"
        }
    }"#;

    let msg: BinanceKlineMessage = serde_json::from_str(json).unwrap();
    let data = msg.into_kline_data().unwrap();
    // T8 FAIL: 此前这里返回 0，现在应返回实际 E 字段值
    assert_eq!(
        data.event_time, 1713900000456,
        "single-stream format must parse E field — if this is 0, the T8 FAIL is still present"
    );
}

#[test]
fn t8_4_single_stream_event_time_missing_defaults_zero() {
    // T8 FAIL fix: 单流格式缺少 E 字段时，回退到 0（向后兼容）
    let json = r#"{
        "e": "kline",
        "s": "BTCUSDT",
        "k": {
            "t": 1713900000000,
            "T": 1713900059999,
            "s": "BTCUSDT",
            "i": "1m",
            "o": "65000.00",
            "h": "65100.00",
            "l": "64900.00",
            "c": "65050.00",
            "v": "100.5",
            "n": 500,
            "x": false,
            "q": "6532500.00"
        }
    }"#;

    let msg: BinanceKlineMessage = serde_json::from_str(json).unwrap();
    let data = msg.into_kline_data().unwrap();
    assert_eq!(data.event_time, 0, "missing E field should default to 0");
}
