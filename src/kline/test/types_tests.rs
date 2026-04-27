use crate::kline::*;

#[test]
fn test_timeframe_ms() {
    assert_eq!(Timeframe::M1.ms(), 60_000);
    assert_eq!(Timeframe::M5.ms(), 300_000);
    assert_eq!(Timeframe::M15.ms(), 900_000);
    assert_eq!(Timeframe::H1.ms(), 3_600_000);
    assert_eq!(Timeframe::H4.ms(), 14_400_000);
    assert_eq!(Timeframe::D1.ms(), 86_400_000);

    // minutes assertions (merged from test_timeframe_minutes)
    assert_eq!(Timeframe::M1.minutes(), 1);
    assert_eq!(Timeframe::M5.minutes(), 5);
    assert_eq!(Timeframe::M15.minutes(), 15);
    assert_eq!(Timeframe::H1.minutes(), 60);
    assert_eq!(Timeframe::H4.minutes(), 240);
    assert_eq!(Timeframe::D1.minutes(), 1440);
}

#[test]
fn test_timeframe_as_str() {
    assert_eq!(Timeframe::M1.as_str(), "1m");
    assert_eq!(Timeframe::M5.as_str(), "5m");
    assert_eq!(Timeframe::M15.as_str(), "15m");
    assert_eq!(Timeframe::H1.as_str(), "1h");
    assert_eq!(Timeframe::H4.as_str(), "4h");
    assert_eq!(Timeframe::D1.as_str(), "1d");

    // Display assertions (merged from test_timeframe_display)
    assert_eq!(format!("{}", Timeframe::M1), "1m");
    assert_eq!(format!("{}", Timeframe::H1), "1h");
    assert_eq!(format!("{}", Timeframe::D1), "1d");
}

#[test]
fn test_timeframe_from_str_lossy() {
    assert_eq!(Timeframe::from_str_lossy("1m"), Some(Timeframe::M1));
    assert_eq!(Timeframe::from_str_lossy("5m"), Some(Timeframe::M5));
    assert_eq!(Timeframe::from_str_lossy("15m"), Some(Timeframe::M15));
    assert_eq!(Timeframe::from_str_lossy("1h"), Some(Timeframe::H1));
    assert_eq!(Timeframe::from_str_lossy("4h"), Some(Timeframe::H4));
    assert_eq!(Timeframe::from_str_lossy("1d"), Some(Timeframe::D1));
    assert_eq!(Timeframe::from_str_lossy("1D"), Some(Timeframe::D1));
    assert_eq!(Timeframe::from_str_lossy("2h"), None);
    assert_eq!(Timeframe::from_str_lossy(""), None);
}


#[test]
fn test_timeframe_default_limit() {
    assert_eq!(Timeframe::M1.default_limit(), 1000);
    assert_eq!(Timeframe::M5.default_limit(), 1000);
    assert_eq!(Timeframe::M15.default_limit(), 1000);
    assert_eq!(Timeframe::H1.default_limit(), 1000);
    assert_eq!(Timeframe::H4.default_limit(), 1000);
    assert_eq!(Timeframe::D1.default_limit(), 1000);
}

#[test]
fn test_timeframe_all() {
    let all = Timeframe::all();
    assert_eq!(all.len(), 6);
    assert!(all.contains(&Timeframe::M1));
    assert!(all.contains(&Timeframe::D1));
}


#[test]
fn test_timeframe_serde() {
    let json = serde_json::to_string(&Timeframe::M1).unwrap();
    assert_eq!(json, "\"1m\"");
    let tf: Timeframe = serde_json::from_str("\"5m\"").unwrap();
    assert_eq!(tf, Timeframe::M5);
}

#[test]
fn test_market_type_display() {
    assert_eq!(format!("{}", MarketType::Spot), "spot");
    assert_eq!(format!("{}", MarketType::Perpetual), "perpetual");
}

#[test]
fn test_market_type_from_str_lossy() {
    assert_eq!(MarketType::from_str_lossy("spot"), MarketType::Spot);
    assert_eq!(MarketType::from_str_lossy("perpetual"), MarketType::Perpetual);
    assert_eq!(MarketType::from_str_lossy("swap"), MarketType::Perpetual);
    assert_eq!(MarketType::from_str_lossy("future"), MarketType::Perpetual);
    assert_eq!(MarketType::from_str_lossy("SPOT"), MarketType::Spot);
}

#[test]
fn test_candle_merge() {
    let mut base = Candle {
        open_time: 0, close_time: 59_999,
        open: 100.0, high: 110.0, low: 95.0, close: 105.0,
        volume: 50.0, quote_volume: 5000.0, trades: 100, closed: false,
    };
    let update = Candle {
        open_time: 0, close_time: 59_999,
        open: 100.0, high: 115.0, low: 90.0, close: 108.0,
        volume: 30.0, quote_volume: 3000.0, trades: 50, closed: true,
    };
    base.merge(&update);
    assert_eq!(base.high, 115.0);
    assert_eq!(base.low, 90.0);
    assert_eq!(base.close, 108.0);
    assert!((base.volume - 80.0).abs() < f64::EPSILON);
    assert!((base.quote_volume - 8000.0).abs() < f64::EPSILON);
    assert_eq!(base.trades, 150);
    assert!(base.closed);
}

#[test]
fn test_candle_from_1m() {
    let base = Candle {
        open_time: 3_600_000, close_time: 3_659_999,
        open: 100.0, high: 110.0, low: 95.0, close: 105.0,
        volume: 50.0, quote_volume: 5000.0, trades: 100, closed: true,
    };
    let h1 = Candle::from_1m(&base, Timeframe::H1);
    assert_eq!(h1.open_time, 3_600_000);
    assert_eq!(h1.close_time, 3_600_000 + 3_600_000 - 1);
    assert_eq!(h1.open, 100.0);
    assert_eq!(h1.high, 110.0);
    assert_eq!(h1.low, 95.0);
    assert_eq!(h1.close, 105.0);
    assert!(!h1.closed);

    // Non-aligned open_time assertion (merged from test_candle_from_1m_alignment)
    let base_unaligned = Candle {
        open_time: 3_630_000, close_time: 3_689_999,
        open: 100.0, high: 110.0, low: 95.0, close: 105.0,
        volume: 50.0, quote_volume: 5000.0, trades: 100, closed: true,
    };
    let h1_unaligned = Candle::from_1m(&base_unaligned, Timeframe::H1);
    assert_eq!(h1_unaligned.open_time, 3_600_000);
}


#[test]
fn test_align_open_time() {
    assert_eq!(align_open_time(0, Timeframe::M5), 0);
    assert_eq!(align_open_time(60_000, Timeframe::M5), 0);
    assert_eq!(align_open_time(300_000, Timeframe::M5), 300_000);
    assert_eq!(align_open_time(3_600_000, Timeframe::H1), 3_600_000);
    assert_eq!(align_open_time(3_630_000, Timeframe::H1), 3_600_000);
    assert_eq!(align_open_time(86_400_000, Timeframe::D1), 86_400_000);
    assert_eq!(align_open_time(90_000_000, Timeframe::D1), 86_400_000);
}

#[test]
fn test_subscription_key() {
    assert_eq!(subscription_key("Binance", "btcusdt"), "binance:BTCUSDT");
    assert_eq!(subscription_key("OKX", "BTC/USDT"), "okx:BTC/USDT");
}

#[test]
fn test_kline_event_type_serde() {
    assert_eq!(serde_json::to_string(&KlineEventType::Update).unwrap(), "\"Update\"");
    assert_eq!(serde_json::to_string(&KlineEventType::Closed).unwrap(), "\"Closed\"");
    assert_eq!(serde_json::to_string(&KlineEventType::Backfilled).unwrap(), "\"Backfilled\"");
}

#[test]
fn test_backtest_range_limit() {
    let m1 = BacktestRangeLimit::for_timeframe(Timeframe::M1);
    assert_eq!(m1.max_days, 7);
    assert_eq!(m1.recommended_days, 3);
    assert!(m1.estimated_candles > 0);

    let d1 = BacktestRangeLimit::for_timeframe(Timeframe::D1);
    assert_eq!(d1.max_days, 1825);
    assert!(d1.estimated_1m_required > d1.estimated_candles);
}

#[test]
fn test_kline_engine_config_default() {
    let config = KlineEngineConfig::default();
    assert!(config.backfill_on_start);
    assert_eq!(config.event_channel_capacity, 8192);
    assert_eq!(config.reconnect_delay_secs, 1);
}

#[test]
fn test_candle_is_closed() {
    let closed = Candle {
        open_time: 0, close_time: 59_999,
        open: 100.0, high: 110.0, low: 90.0, close: 105.0,
        volume: 50.0, quote_volume: 5000.0, trades: 100, closed: true,
    };
    assert!(closed.is_closed());

    let unclosed = Candle {
        open_time: 0, close_time: 59_999,
        open: 100.0, high: 110.0, low: 90.0, close: 105.0,
        volume: 50.0, quote_volume: 5000.0, trades: 100, closed: false,
    };
    assert!(!unclosed.is_closed());
}
