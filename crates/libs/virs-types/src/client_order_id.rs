use chrono::Utc;
use uuid::Uuid;


fn generate_hash(bot_id: Uuid) -> String {
    let nanos = Utc::now().timestamp_nanos_opt().unwrap_or(0) as u32;
    let bytes = bot_id.as_bytes();
    let mut hash_bytes = [0u8; 4];
    for i in 0..4 {
        hash_bytes[i] = bytes[i]
            ^ bytes[i + 4]
            ^ bytes[i + 8]
            ^ bytes[i + 12]
            ^ ((nanos >> (i * 8)) & 0xFF) as u8;
    }
    format!(
        "{:02X}{:02X}{:02X}{:02X}",
        hash_bytes[0], hash_bytes[1], hash_bytes[2], hash_bytes[3]
    )
}


fn timestamp_str() -> String {
    Utc::now().format("%Y%m%d%H%M%S").to_string()
}


pub fn format_auto_open(bot_id: Uuid, side: &str) -> String {
    let f3 = match side {
        "long" => "L",
        "short" => "S",
        _ => "X",
    };
    format!("AO{}__{}{}", f3, timestamp_str(), generate_hash(bot_id))
}


pub fn format_auto_close(bot_id: Uuid, side: &str) -> String {
    let f3 = match side {
        "long" => "L",
        "short" => "S",
        _ => "X",
    };
    format!("AC{}__{}{}", f3, timestamp_str(), generate_hash(bot_id))
}
