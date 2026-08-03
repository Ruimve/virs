use crate::adapter::binance::kline_ws::{
    binance_ws_symbol, BinanceKlineData, BinanceKlineInner, BinanceKlineMessage, KlineWs,
    KLINE_WS_DELAY_THRESHOLD_MS,
};
use crate::ws_types::KlineWsClient;

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

    let result: Result<BinanceKlineMessage, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

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
    assert!(
        result.is_err(),
        "invalid OHLCV fields must return Err, not 0.0"
    );
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

#[test]
fn test_binance_ws_symbol_basic() {
    assert_eq!(binance_ws_symbol("BTCUSDT"), "btcusdt");

    assert_eq!(binance_ws_symbol("BTC/USDT"), "btcusdt");

    assert_eq!(binance_ws_symbol("btcusdt"), "btcusdt");
}

#[tokio::test]
async fn test_subscribe_without_start() {
    let ws = KlineWs::new_perpetual(None);
    assert!(!ws.is_running());

    ws.subscribe("BTCUSDT").await;

    let subs = ws.handler.subscriptions.read().await;
    assert!(subs.contains(&"btcusdt@kline_1m".to_string()));

    let map = ws.handler.symbol_map.read().await;
    assert_eq!(map.get("btcusdt").unwrap(), "BTCUSDT");

    assert!(!ws.is_running());
}

#[test]
fn t8_1_event_time_parsed_and_accessible() {
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

    assert_eq!(data.event_time, 1713900000123);
}

#[test]
fn t8_2_delay_threshold_is_5000ms() {
    assert_eq!(KLINE_WS_DELAY_THRESHOLD_MS, 5_000);

    let event_time = 1713900000000_i64;
    let local_now = 1713900006000_i64;
    let delay_ms = local_now - event_time;
    assert!(delay_ms > KLINE_WS_DELAY_THRESHOLD_MS);

    let local_now_ok = 1713900003000_i64;
    let delay_ok = local_now_ok - event_time;
    assert!(delay_ok <= KLINE_WS_DELAY_THRESHOLD_MS);
}

#[test]
fn t8_3_single_stream_event_time_parsed() {
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

    assert_eq!(
        data.event_time, 1713900000456,
        "single-stream format must parse E field — if this is 0, the T8 FAIL is still present"
    );
}

#[test]
fn t8_4_single_stream_event_time_missing_returns_none() {
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
    let result = msg.into_kline_data();
    assert!(result.is_none(), "missing E field should return None");
}
