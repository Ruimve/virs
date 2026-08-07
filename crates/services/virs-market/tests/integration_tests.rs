use virs_market::{
    align_open_time, candle_from_1m, subscription_key, timeframe_str_to_ms,
};
use virs_type::{Candle, Timeframe};

const BASE: i64 = 1_700_000_040_000;

fn make_1m(open_time: i64, open: f64, high: f64, low: f64, close: f64, closed: bool) -> Candle {
    Candle {
        open_time,
        close_time: open_time + 59_999,
        open,
        high,
        low,
        close,
        volume: 100.0,
        quote_volume: 100.0 * close,
        trades: 10,
        closed,
    }
}

#[test]
fn int_2_1_candle_from_1m_basic() {
    let c1 = make_1m(BASE, 100.0, 102.0, 98.0, 101.0, true);
    let from_1m = candle_from_1m(&c1, Timeframe::M5);
    assert_eq!(from_1m.open_time % Timeframe::M5.ms(), 0);
    assert_eq!(from_1m.open, 100.0);
    assert_eq!(from_1m.close, 101.0);
}

#[test]
fn int_3_1_subscription_key_then_check() {
    let key1 = subscription_key("binance", "BTCUSDT");
    let key2 = subscription_key("binance", "BTCUSDT");
    assert_eq!(key1, key2);
    assert!(key1.contains(':'));
    assert!(key1.starts_with("binance:"));
}

#[test]
fn int_3_2_align_multi_timeframe() {
    let time = BASE + 123_456;
    let m1 = align_open_time(time, Timeframe::M1);
    let m5 = align_open_time(time, Timeframe::M5);
    let h1 = align_open_time(time, Timeframe::H1);
    let d1 = align_open_time(time, Timeframe::D1);

    assert!(m5 <= m1);
    assert!(h1 <= m5);
    assert!(d1 <= h1);
    assert_eq!(m1 % 60_000, 0);
    assert_eq!(m5 % 300_000, 0);
    assert_eq!(h1 % 3_600_000, 0);
    assert_eq!(d1 % 86_400_000, 0);
}

#[test]
fn int_6_1_timeframe_str_to_ms() {
    assert_eq!(timeframe_str_to_ms("1m"), 60_000);
    assert_eq!(timeframe_str_to_ms("5m"), 300_000);
    assert_eq!(timeframe_str_to_ms("15m"), 900_000);
    assert_eq!(timeframe_str_to_ms("1h"), 3_600_000);
    assert_eq!(timeframe_str_to_ms("4h"), 14_400_000);
    assert_eq!(timeframe_str_to_ms("1d"), 86_400_000);

    assert_eq!(timeframe_str_to_ms("invalid"), 60_000);
}
