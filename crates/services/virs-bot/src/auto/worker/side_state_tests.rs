use virs_types::enums::{PositionSide, PositionStatus};
use virs_types::position::Position;

use super::side_state::{PendingClose, PendingOpen, SideState};

// ===== helpers =====

fn make_position(side: PositionSide, status: PositionStatus, quantity: f64) -> Position {
    Position {
        id: uuid::Uuid::new_v4(),
        exchange: "binance".to_string(),
        symbol: "BTCUSDT".to_string(),
        side,
        status,
        quantity,
        entry_price: 100.0,
        realized_pnl: 0.0,
        client_order_id: Some("test_oid".to_string()),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

fn make_pending_open() -> PendingOpen {
    PendingOpen {
        side: "long".to_string(),
        entry_price: 100.0,
        position_size: 1.0,
        stop_loss: 97.0,
        take_profit: 106.0,
        client_order_id: "AOL__test".to_string(),
        sent_at: tokio::time::Instant::now(),
    }
}

fn make_pending_close() -> PendingClose {
    PendingClose {
        side: "long".to_string(),
        close_reason: "stop_loss".to_string(),
        entry_price: 100.0,
        position_size: 1.0,
        client_order_id: "ACL__test".to_string(),
        sent_at: tokio::time::Instant::now(),
    }
}

// ===== has_position =====

#[test]
fn ss_1_1_has_position_open_with_quantity() {
    let mut s = SideState::default();
    s.position = Some(make_position(PositionSide::Long, PositionStatus::Open, 1.5));
    assert!(s.has_position());
}

#[test]
fn ss_1_2_has_position_open_zero_quantity() {
    let mut s = SideState::default();
    s.position = Some(make_position(PositionSide::Long, PositionStatus::Open, 0.0));
    assert!(!s.has_position());
}

#[test]
fn ss_1_3_has_position_open_near_zero_quantity() {
    let mut s = SideState::default();
    s.position = Some(make_position(PositionSide::Short, PositionStatus::Open, 1e-10));
    assert!(!s.has_position(), "quantity below epsilon should not count as open");
}

#[test]
fn ss_1_4_has_position_opening_status() {
    let mut s = SideState::default();
    s.position = Some(make_position(PositionSide::Long, PositionStatus::Opening, 1.0));
    assert!(!s.has_position(), "Opening status is not Open");
}

#[test]
fn ss_1_5_has_position_closing_status() {
    let mut s = SideState::default();
    s.position = Some(make_position(PositionSide::Long, PositionStatus::Closing, 1.0));
    assert!(!s.has_position(), "Closing status is not Open");
}

#[test]
fn ss_1_6_has_position_none() {
    let s = SideState::default();
    assert!(!s.has_position());
}

#[test]
fn ss_1_7_has_position_closed_status() {
    let mut s = SideState::default();
    s.position = Some(make_position(PositionSide::Long, PositionStatus::Closed, 1.0));
    assert!(!s.has_position());
}

// ===== is_pending =====

#[test]
fn ss_2_1_is_pending_open_only() {
    let mut s = SideState::default();
    s.pending_open = Some(make_pending_open());
    assert!(s.is_pending());
}

#[test]
fn ss_2_2_is_pending_close_only() {
    let mut s = SideState::default();
    s.pending_close = Some(make_pending_close());
    assert!(s.is_pending());
}

#[test]
fn ss_2_3_is_pending_both() {
    let mut s = SideState::default();
    s.pending_open = Some(make_pending_open());
    s.pending_close = Some(make_pending_close());
    assert!(s.is_pending());
}

#[test]
fn ss_2_4_is_pending_none() {
    let s = SideState::default();
    assert!(!s.is_pending());
}

// ===== get_position =====

#[test]
fn ss_3_1_get_position_some() {
    let mut s = SideState::default();
    let pos = make_position(PositionSide::Long, PositionStatus::Open, 2.0);
    s.position = Some(pos.clone());
    let got = s.get_position().expect("should be Some");
    assert_eq!(got.id, pos.id);
    assert_eq!(got.quantity, 2.0);
}

#[test]
fn ss_3_2_get_position_none() {
    let s = SideState::default();
    assert!(s.get_position().is_none());
}

// ===== clear_position =====

#[test]
fn ss_4_1_clear_position_resets_all_position_fields() {
    let mut s = SideState::default();
    s.position = Some(make_position(PositionSide::Long, PositionStatus::Open, 1.0));
    s.stop_loss = 97.0;
    s.take_profit = 106.0;
    s.position_opened_at = Some(tokio::time::Instant::now());
    s.open_client_order_id = Some("AOL__test".to_string());
    s.open_fee = 0.5;
    s.log_id = Some(uuid::Uuid::new_v4());

    s.clear_position();

    assert!(s.position.is_none());
    assert_eq!(s.stop_loss, 0.0);
    assert_eq!(s.take_profit, 0.0);
    assert!(s.position_opened_at.is_none());
    assert!(s.open_client_order_id.is_none());
    assert_eq!(s.open_fee, 0.0);
}

#[test]
fn ss_4_2_clear_position_preserves_log_id() {
    let mut s = SideState::default();
    let log_id = uuid::Uuid::new_v4();
    s.log_id = Some(log_id);
    s.position = Some(make_position(PositionSide::Long, PositionStatus::Open, 1.0));

    s.clear_position();

    assert_eq!(s.log_id, Some(log_id), "log_id must survive clear_position");
}

#[test]
fn ss_4_3_clear_position_preserves_last_close_event() {
    let mut s = SideState::default();
    let event = ("long".to_string(), "stop_loss".to_string(), chrono::Utc::now());
    s.last_close_event = Some(event.clone());
    s.position = Some(make_position(PositionSide::Long, PositionStatus::Open, 1.0));

    s.clear_position();

    assert_eq!(s.last_close_event, Some(event), "last_close_event must survive clear_position");
}

#[test]
fn ss_4_4_clear_position_on_default_is_noop() {
    let mut s = SideState::default();
    s.clear_position();
    assert!(s.position.is_none());
    assert_eq!(s.stop_loss, 0.0);
}

// ===== clear_on_close =====

#[test]
fn ss_5_1_clear_on_close_resets_position_fields() {
    let mut s = SideState::default();
    s.position = Some(make_position(PositionSide::Long, PositionStatus::Open, 1.0));
    s.stop_loss = 97.0;
    s.take_profit = 106.0;
    s.position_opened_at = Some(tokio::time::Instant::now());
    s.open_client_order_id = Some("AOL__test".to_string());
    s.open_fee = 0.5;

    let event = ("long".to_string(), "take_profit".to_string(), chrono::Utc::now());
    s.clear_on_close(event.clone());

    assert!(s.position.is_none());
    assert_eq!(s.stop_loss, 0.0);
    assert_eq!(s.take_profit, 0.0);
    assert!(s.position_opened_at.is_none());
    assert!(s.open_client_order_id.is_none());
    assert_eq!(s.open_fee, 0.0);
}

#[test]
fn ss_5_2_clear_on_close_clears_log_id() {
    let mut s = SideState::default();
    s.log_id = Some(uuid::Uuid::new_v4());
    s.position = Some(make_position(PositionSide::Long, PositionStatus::Open, 1.0));

    s.clear_on_close(("long".to_string(), "stop_loss".to_string(), chrono::Utc::now()));

    assert!(s.log_id.is_none(), "log_id must be cleared by clear_on_close");
}

#[test]
fn ss_5_3_clear_on_close_sets_last_close_event() {
    let mut s = SideState::default();
    s.position = Some(make_position(PositionSide::Long, PositionStatus::Open, 1.0));

    let event = ("short".to_string(), "llm_decision".to_string(), chrono::Utc::now());
    s.clear_on_close(event.clone());

    assert_eq!(s.last_close_event, Some(event));
}

#[test]
fn ss_5_4_clear_on_close_overwrites_previous_last_close_event() {
    let mut s = SideState::default();
    s.last_close_event = Some(("long".to_string(), "stop_loss".to_string(), chrono::Utc::now()));
    s.position = Some(make_position(PositionSide::Long, PositionStatus::Open, 1.0));

    let new_event = ("long".to_string(), "take_profit".to_string(), chrono::Utc::now());
    s.clear_on_close(new_event.clone());

    assert_eq!(s.last_close_event, Some(new_event));
}

// ===== Default trait =====

#[test]
fn ss_6_1_default_all_fields_zero_or_none() {
    let s = SideState::default();
    assert!(s.position.is_none());
    assert!(s.pending_open.is_none());
    assert!(s.pending_close.is_none());
    assert_eq!(s.stop_loss, 0.0);
    assert_eq!(s.take_profit, 0.0);
    assert!(s.position_opened_at.is_none());
    assert!(s.open_client_order_id.is_none());
    assert!(s.log_id.is_none());
    assert_eq!(s.open_fee, 0.0);
    assert!(s.last_close_event.is_none());
}
