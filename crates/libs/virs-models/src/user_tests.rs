//! Unit tests for user.rs User::to_response.

use chrono::Utc;
use uuid::Uuid;

use crate::{User, UserRole, UserResponse};

fn make_user(email: Option<&str>) -> User {
    User {
        id: Uuid::nil(),
        username: "testuser".into(),
        password_hash: "$2b$12$secret_hash".into(),
        role: UserRole::Admin,
        email: email.map(|e| e.into()),
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

// ============================================================
// TC-U1: to_response
// ============================================================

#[test]
fn u1_1_normal_conversion() {
    let user = make_user(Some("user@example.com"));
    let response: UserResponse = user.to_response();

    assert_eq!(response.id, user.id);
    assert_eq!(response.username, user.username);
    assert_eq!(response.role, user.role);
    assert_eq!(response.email, user.email);
    assert_eq!(response.is_active, user.is_active);
    assert_eq!(response.created_at, user.created_at);
    // UserResponse should NOT have password_hash field
    // (guaranteed by struct definition — no field exists to check)
}

#[test]
fn u1_2_email_none() {
    let user = make_user(None);
    let response = user.to_response();
    assert_eq!(response.email, None);
}
