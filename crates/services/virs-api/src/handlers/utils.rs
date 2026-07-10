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


