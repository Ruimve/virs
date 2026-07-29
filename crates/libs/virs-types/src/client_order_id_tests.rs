use crate::client_order_id::{format_auto_close, format_auto_open};
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
fn test_uniqueness() {
    let bot_id = Uuid::new_v4();
    let mut cids = std::collections::HashSet::new();
    for _ in 0..1000 {
        let cid = format_auto_open(bot_id, "long");
        cids.insert(cid);
    }

    assert_eq!(cids.len(), 1000, "client_order_id should be unique");
}
