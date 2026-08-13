use chrono::Utc;
use uuid::Uuid;


/* 生成client_order_id哈希后缀：将bot_id与当前时间纳秒异或，确保唯一性和可追溯性 */
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


/* 生成开仓client_order_id：格式为AO{L|S}__{时间戳}{哈希}，L=多/S=空 */
pub fn format_bot_open(bot_id: Uuid, side: &str) -> String {
    let f3 = match side {
        "long" => "L",
        "short" => "S",
        _ => "X",
    };
    format!("AO{}__{}{}", f3, timestamp_str(), generate_hash(bot_id))
}


/* 生成平仓client_order_id：格式为AC{L|S}__{时间戳}{哈希}，L=多/S=空 */
pub fn format_bot_close(bot_id: Uuid, side: &str) -> String {
    let f3 = match side {
        "long" => "L",
        "short" => "S",
        _ => "X",
    };
    format!("AC{}__{}{}", f3, timestamp_str(), generate_hash(bot_id))
}
