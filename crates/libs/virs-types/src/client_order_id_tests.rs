use crate::client_order_id::{format_auto_close, format_auto_open, format_grid_order, parse_grid_order};
use uuid::Uuid;

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
