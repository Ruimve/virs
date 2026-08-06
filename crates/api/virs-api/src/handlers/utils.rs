use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct TradesQuery {
    pub page: u32,
    pub page_size: u32,
}

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
