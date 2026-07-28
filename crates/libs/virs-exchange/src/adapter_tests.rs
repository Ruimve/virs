use virs_ccxt::CcxtKline;
use virs_types::market::Kline;

use crate::adapter::to_models_kline;

#[test]
fn a4_1_kline_normal_conversion() {
    let ck = CcxtKline {
        timestamp: 1700000000000,
        close_time: None,
        open: 50000.0,
        high: 51000.0,
        low: 49000.0,
        close: 50500.0,
        volume: 1000.0,
        quote_volume: Some(50000000.0),
        trades: Some(5000),
    };
    let kline: Kline = to_models_kline(ck, "BTC/USDT", "binance", "1m");
    assert_eq!(kline.open_time, 1700000000000);
    assert_eq!(kline.open, 50000.0);
    assert_eq!(kline.high, 51000.0);
    assert_eq!(kline.low, 49000.0);
    assert_eq!(kline.close, 50500.0);
    assert_eq!(kline.volume, 1000.0);
    assert_eq!(kline.quote_volume, 50000000.0);
    assert_eq!(kline.trades, 5000);
    assert_eq!(kline.symbol, "BTC/USDT");
    assert_eq!(kline.exchange, "binance");
    assert_eq!(kline.interval, "1m");

    assert_eq!(kline.close_time, 1700000000000 + 60_000 - 1);
}

#[test]
fn a4_2_kline_exchange_field() {
    let ck = CcxtKline {
        timestamp: 100,
        close_time: None,
        open: 1.0,
        high: 2.0,
        low: 0.5,
        close: 1.5,
        volume: 10.0,
        quote_volume: None,
        trades: None,
    };
    let kline = to_models_kline(ck, "ETH/USDC", "okx", "1h");
    assert_eq!(kline.exchange, "okx");
    assert_eq!(kline.symbol, "ETH/USDC");
    assert_eq!(kline.interval, "1h");

    assert_eq!(kline.quote_volume, 0.0);
    assert_eq!(kline.trades, 0);

    assert_eq!(kline.close_time, 100 + 3_600_000 - 1);
}

#[test]
fn a4_3_kline_close_time_binance_format() {
    let intervals: &[(&str, i64)] = &[
        ("1m", 60_000),
        ("5m", 300_000),
        ("15m", 900_000),
        ("30m", 1_800_000),
        ("1h", 3_600_000),
        ("4h", 14_400_000),
        ("1d", 86_400_000),
        ("1w", 604_800_000),
    ];
    for (interval, tf_ms) in intervals {
        let ck = CcxtKline {
            timestamp: 1700000000000,
            close_time: None,
            open: 50000.0,
            high: 51000.0,
            low: 49000.0,
            close: 50500.0,
            volume: 1000.0,
            quote_volume: Some(50000000.0),
            trades: Some(5000),
        };
        let kline = to_models_kline(ck, "BTC/USDT", "binance", interval);
        assert_eq!(
            kline.close_time,
            1700000000000 + tf_ms - 1,
            "close_time mismatch for interval {}",
            interval
        );

        assert_eq!(
            kline.close_time - kline.open_time,
            tf_ms - 1,
            "close_time - open_time must equal interval_ms - 1 for interval {}",
            interval
        );
    }
}

#[test]
fn a4_4_kline_close_time_from_exchange() {
    let ck = CcxtKline {
        timestamp: 1700000000000,
        close_time: Some(1700000059000),
        open: 50000.0,
        high: 51000.0,
        low: 49000.0,
        close: 50500.0,
        volume: 1000.0,
        quote_volume: Some(50000000.0),
        trades: Some(5000),
    };
    let kline = to_models_kline(ck, "BTC/USDT", "binance", "1m");

    assert_eq!(kline.close_time, 1700000059000);
    assert_ne!(kline.close_time, 1700000000000 + 60_000 - 1);
}
