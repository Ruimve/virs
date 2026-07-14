use crate::types::*;


#[test]
fn t1_1_timeframe_all_count() {
    let all = Timeframe::all();
    assert_eq!(all.len(), 6);
}

#[test]
fn t1_2_timeframe_as_str() {
    assert_eq!(Timeframe::M1.as_str(), "1m");
    assert_eq!(Timeframe::M5.as_str(), "5m");
    assert_eq!(Timeframe::M15.as_str(), "15m");
    assert_eq!(Timeframe::H1.as_str(), "1h");
    assert_eq!(Timeframe::H4.as_str(), "4h");
    assert_eq!(Timeframe::D1.as_str(), "1d");
}

#[test]
fn t1_3_timeframe_ms() {
    assert_eq!(Timeframe::M1.ms(), 60_000);
    assert_eq!(Timeframe::M5.ms(), 300_000);
    assert_eq!(Timeframe::M15.ms(), 900_000);
    assert_eq!(Timeframe::H1.ms(), 3_600_000);
    assert_eq!(Timeframe::H4.ms(), 14_400_000);
    assert_eq!(Timeframe::D1.ms(), 86_400_000);
}

#[test]
fn t1_5_timeframe_default_limit() {
    for tf in Timeframe::all() {
        assert_eq!(tf.default_limit(), 1000);
    }
}

#[test]
fn t1_7_timeframe_display() {
    assert_eq!(format!("{}", Timeframe::M1), "1m");
    assert_eq!(format!("{}", Timeframe::H1), "1h");
    assert_eq!(format!("{}", Timeframe::D1), "1d");
}


#[test]
fn t2_1_subscription_key_basic() {
    let key = subscription_key("binance", "BTC/USDT");
    assert_eq!(key, "binance:BTC/USDT");
}

#[test]
fn t2_2_subscription_key_case() {
    let key = subscription_key("Binance", "btc/usdt");
    assert_eq!(key, "binance:BTC/USDT");
}


#[test]
fn t3_1_align_open_time_m1() {

    let base = (1700000000500_i64 / 60_000) * 60_000;
    let aligned = align_open_time(1700000000500, Timeframe::M1);
    assert_eq!(aligned, base);
    assert_eq!(aligned % 60_000, 0);
}

#[test]
fn t3_2_align_open_time_h1() {

    let aligned = align_open_time(1700001234567, Timeframe::H1);
    assert_eq!(aligned % 3_600_000, 0);
}

#[test]
fn t3_3_align_open_time_d1() {

    let aligned = align_open_time(1700001234567, Timeframe::D1);
    assert_eq!(aligned % 86_400_000, 0);
}

#[test]
fn t3_4_align_open_time_exact() {

    let exact = 1_700_000_040_000;
    assert_eq!(exact % 60_000, 0);
    let aligned = align_open_time(exact, Timeframe::M1);
    assert_eq!(aligned, exact);
}
