//! 统一 client_order_id 格式化与解析模块
//!
//! 格式: `{F1}{F2}{F3}{F4}{YYYYMMDDHHmmss}{HASH}` 共 27 字符
//!
//! | 位置 | 字段 | 含义 |
//! |------|------|------|
//! | F1 (1字符) | Bot 类型 | A=Auto, G=Grid |
//! | F2 (1字符) | 操作 | O=Open(开仓), C=Close(平仓) |
//! | F3 (1字符) | 方向 | L=Long, S=Short |
//! | F4 (2字符) | 网格层级 | 01-99, 非网格用 `__` |
//! | 日期 (14字符) | 下单时间 | YYYYMMDDHHmmss |
//! | HASH (8字符) | 唯一哈希 | 基于 bot_id + nanos 的 8 位 hex |
//!
//! 约束: 总长 27 字符 ≤ 36 (币安限制), 字符集 `[A-Z0-9_]`

use chrono::Utc;
use uuid::Uuid;

/// 生成 8 字符 hex 哈希，基于 bot_id 和当前纳秒时间戳
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

/// 生成时间戳字符串 YYYYMMDDHHmmss
fn timestamp_str() -> String {
    Utc::now().format("%Y%m%d%H%M%S").to_string()
}

/// 生成 Auto bot 开仓的 client_order_id
///
/// `side` 为 "long" 或 "short"
pub fn format_auto_open(bot_id: Uuid, side: &str) -> String {
    let f3 = match side {
        "long" => "L",
        "short" => "S",
        _ => "X",
    };
    format!("AO{}__{}{}", f3, timestamp_str(), generate_hash(bot_id))
}

/// 生成 Auto bot 平仓的 client_order_id
///
/// `side` 为 "long" 或 "short" (持仓方向)
pub fn format_auto_close(bot_id: Uuid, side: &str) -> String {
    let f3 = match side {
        "long" => "L",
        "short" => "S",
        _ => "X",
    };
    format!("AC{}__{}{}", f3, timestamp_str(), generate_hash(bot_id))
}

/// 生成 Grid bot 订单的 client_order_id
///
/// `level` 为网格层级 (1-99)
/// `is_open` 为 true=开仓, false=平仓
/// `position_side` 为 "long" 或 "short"
pub fn format_grid_order(bot_id: Uuid, level: i32, is_open: bool, position_side: &str) -> String {
    let f2 = if is_open { "O" } else { "C" };
    let f3 = match position_side {
        "long" => "L",
        "short" => "S",
        _ => "X",
    };
    format!(
        "G{}{}{:02}{}{}",
        f2,
        f3,
        level,
        timestamp_str(),
        generate_hash(bot_id)
    )
}

/// Grid worker 解析 client_order_id，提取 (level, "buy"/"sell")
///
/// 从 (Open/Close, Long/Short) 推导订单方向:
/// - Open Long → buy
/// - Close Long → sell
/// - Open Short → sell
/// - Close Short → buy
pub fn parse_grid_order(coi: &str) -> Option<(usize, String)> {
    if coi.len() < 5 || !coi.starts_with('G') {
        return None;
    }
    let op = coi.chars().nth(1)?;
    let dir = coi.chars().nth(2)?;
    let level_str = &coi[3..5];
    let level_idx = level_str.parse::<usize>().ok()?;

    let side = match (op, dir) {
        ('O', 'L') => "buy",
        ('C', 'L') => "sell",
        ('O', 'S') => "sell",
        ('C', 'S') => "buy",
        _ => return None,
    };
    Some((level_idx, side.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_open_long() {
        let bot_id = Uuid::new_v4();
        let cid = format_auto_open(bot_id, "long");
        assert!(cid.starts_with("AOL__"));
        assert_eq!(cid.len(), 27);
    }

    #[test]
    fn test_auto_open_short() {
        let bot_id = Uuid::new_v4();
        let cid = format_auto_open(bot_id, "short");
        assert!(cid.starts_with("AOS__"));
        assert_eq!(cid.len(), 27);
    }

    #[test]
    fn test_auto_close_long() {
        let bot_id = Uuid::new_v4();
        let cid = format_auto_close(bot_id, "long");
        assert!(cid.starts_with("ACL__"));
        assert_eq!(cid.len(), 27);
    }

    #[test]
    fn test_auto_close_short() {
        let bot_id = Uuid::new_v4();
        let cid = format_auto_close(bot_id, "short");
        assert!(cid.starts_with("ACS__"));
        assert_eq!(cid.len(), 27);
    }

    #[test]
    fn test_grid_open_long() {
        let bot_id = Uuid::new_v4();
        let cid = format_grid_order(bot_id, 1, true, "long");
        assert!(cid.starts_with("GOL01"));
        assert_eq!(cid.len(), 27);
        let (level, side) = parse_grid_order(&cid).unwrap();
        assert_eq!(level, 1);
        assert_eq!(side, "buy");
    }

    #[test]
    fn test_grid_close_long() {
        let bot_id = Uuid::new_v4();
        let cid = format_grid_order(bot_id, 5, false, "long");
        assert!(cid.starts_with("GCL05"));
        let (level, side) = parse_grid_order(&cid).unwrap();
        assert_eq!(level, 5);
        assert_eq!(side, "sell");
    }

    #[test]
    fn test_grid_open_short() {
        let bot_id = Uuid::new_v4();
        let cid = format_grid_order(bot_id, 10, true, "short");
        assert!(cid.starts_with("GOS10"));
        let (level, side) = parse_grid_order(&cid).unwrap();
        assert_eq!(level, 10);
        assert_eq!(side, "sell");
    }

    #[test]
    fn test_grid_close_short() {
        let bot_id = Uuid::new_v4();
        let cid = format_grid_order(bot_id, 99, false, "short");
        assert!(cid.starts_with("GCS99"));
        let (level, side) = parse_grid_order(&cid).unwrap();
        assert_eq!(level, 99);
        assert_eq!(side, "buy");
    }

    #[test]
    fn test_uniqueness() {
        let bot_id = Uuid::new_v4();
        let mut cids = std::collections::HashSet::new();
        for _ in 0..1000 {
            let cid = format_auto_open(bot_id, "long");
            cids.insert(cid);
        }
        // 纳秒精度足够保证 1000 次生成不碰撞
        assert_eq!(cids.len(), 1000, "client_order_id should be unique");
    }

    #[test]
    fn test_parse_non_grid_returns_none() {
        assert!(parse_grid_order("AOL__20260711143052A1B2C3D4").is_none());
    }

    #[test]
    fn test_parse_invalid_returns_none() {
        assert!(parse_grid_order("GXX0020260711143052A1B2C3D4").is_none());
    }
}
