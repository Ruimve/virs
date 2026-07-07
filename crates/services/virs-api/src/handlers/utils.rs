//! Shared utility functions for API handlers.

/// Format a duration in milliseconds as a human-readable string.
///
/// Examples:
/// - `0` or negative → `"-"`
/// - `5000` → `"5s"`
/// - `65000` → `"1m5s"`
/// - `3661000` → `"1h1m"`
pub fn format_duration(ms: i64) -> String {
    if ms <= 0 {
        return "-".to_string();
    }
    let seconds = ms / 1000;
    let minutes = seconds / 60;
    let hours = minutes / 60;
    if hours > 0 {
        format!("{}h{}m", hours, minutes % 60)
    } else if minutes > 0 {
        format!("{}m{}s", minutes, seconds % 60)
    } else {
        format!("{}s", seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::format_duration;

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
}
