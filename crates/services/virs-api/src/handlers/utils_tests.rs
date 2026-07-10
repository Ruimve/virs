use super::utils::format_duration;

#[test]
fn t13_1_format_duration_zero_or_negative() {
    assert_eq!(format_duration(0), "-");
    assert_eq!(format_duration(-1), "-");
    assert_eq!(format_duration(-1000), "-");
}

#[test]
fn t13_2_format_duration_seconds() {
    assert_eq!(format_duration(1000), "1s");
    assert_eq!(format_duration(5000), "5s");
    assert_eq!(format_duration(59999), "59s");
}

#[test]
fn t13_3_format_duration_minutes() {
    assert_eq!(format_duration(60000), "1m0s");
    assert_eq!(format_duration(65000), "1m5s");
    assert_eq!(format_duration(3599999), "59m59s");
}

#[test]
fn t13_4_format_duration_hours() {
    assert_eq!(format_duration(3600000), "1h0m");
    assert_eq!(format_duration(3661000), "1h1m");
    assert_eq!(format_duration(7320000), "2h2m");
}
