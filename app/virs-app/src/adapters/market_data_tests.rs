use crate::adapters::market_data::candle_to_kline;
use virs_type::Candle;

fn make_candle() -> Candle {
    Candle {
        open_time: 1700000000000,
        close_time: 1700000059999,
        open: 100.0,
        high: 105.0,
        low: 95.0,
        close: 102.0,
        volume: 500.0,
        quote_volume: 51000.0,
        trades: 120,
        closed: true,
    }
}

#[test]
fn m1_1_candle_to_kline_basic() {
    let c = make_candle();
    let k = candle_to_kline(&c);
    assert!((k.open - 100.0).abs() < 1e-10);
    assert!((k.high - 105.0).abs() < 1e-10);
    assert!((k.low - 95.0).abs() < 1e-10);
    assert!((k.close - 102.0).abs() < 1e-10);
    assert!((k.volume - 500.0).abs() < 1e-10);
    assert!((k.quote_volume - 51000.0).abs() < 1e-10);
    assert_eq!(k.trades, 120);
}

#[test]
fn m1_2_candle_to_kline_timestamps() {
    let c = make_candle();
    let k = candle_to_kline(&c);
    assert_eq!(k.open_time, 1700000000000);
    assert_eq!(k.close_time, 1700000059999);
}

#[test]
fn m1_3_candle_to_kline_zero_values() {
    let c = Candle {
        open_time: 0,
        close_time: 0,
        open: 0.0,
        high: 0.0,
        low: 0.0,
        close: 0.0,
        volume: 0.0,
        quote_volume: 0.0,
        trades: 0,
        closed: false,
    };
    let k = candle_to_kline(&c);
    assert!((k.open - 0.0).abs() < 1e-10);
    assert!((k.high - 0.0).abs() < 1e-10);
    assert!((k.low - 0.0).abs() < 1e-10);
    assert!((k.close - 0.0).abs() < 1e-10);
    assert_eq!(k.trades, 0);
}

#[test]
fn m1_4_candle_to_kline_metadata_empty() {
    let c = make_candle();
    let k = candle_to_kline(&c);

    assert!(k.symbol.is_empty());
    assert!(k.exchange.is_empty());
    assert!(k.interval.is_empty());
}
