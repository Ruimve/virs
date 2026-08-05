use virs_type::CcxtOrderStatus;
use virs_type::OrderStatus;

#[test]
fn t1_1_new_to_open() {
    let status: OrderStatus = CcxtOrderStatus::New.into();
    assert_eq!(status, OrderStatus::Open);
}

#[test]
fn t1_2_partially_filled() {
    let status: OrderStatus = CcxtOrderStatus::PartiallyFilled.into();
    assert_eq!(status, OrderStatus::PartiallyFilled);
}

#[test]
fn t1_3_filled() {
    let status: OrderStatus = CcxtOrderStatus::Filled.into();
    assert_eq!(status, OrderStatus::Filled);
}

#[test]
fn t1_4_canceled() {
    let status: OrderStatus = CcxtOrderStatus::Canceled.into();
    assert_eq!(status, OrderStatus::Canceled);
}

#[test]
fn t1_5_expired() {
    let status: OrderStatus = CcxtOrderStatus::Expired.into();
    assert_eq!(status, OrderStatus::Expired);
}

#[test]
fn t1_6_expired_in_match() {
    let status: OrderStatus = CcxtOrderStatus::ExpiredInMatch.into();
    assert_eq!(status, OrderStatus::Expired);
}

#[test]
fn t7_1_funding_time_zero_is_epoch() {
    let result = chrono::DateTime::from_timestamp_millis(0);
    assert!(result.is_some(), "timestamp 0 is a valid DateTime (epoch)");
    assert_eq!(
        result.unwrap(),
        chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap()
    );
}

#[test]
fn t7_2_filter_zero_before_from_timestamp_millis() {
    let raw_ts: i64 = 0;
    let filtered = Some(raw_ts).filter(|&ts| ts > 0);
    assert_eq!(filtered, None, "0 should be filtered out");

    let valid_ts: i64 = 1700000000000;
    let valid_filtered = Some(valid_ts).filter(|&ts| ts > 0);
    assert_eq!(valid_filtered, Some(1700000000000));
}
