use crate::adapters::utils::{derive_open_side, sanitize_pnl_pct};

#[test]
fn u1_1_sanitize_pnl_pct_normal() {
    let result = sanitize_pnl_pct(0.15);
    assert!((result - 0.15).abs() < 1e-10);
}

#[test]
fn u1_2_sanitize_pnl_pct_nan() {
    let result = sanitize_pnl_pct(f64::NAN);
    assert!((result - 0.0).abs() < 1e-10);
}

#[test]
fn u1_3_sanitize_pnl_pct_zero() {
    let result = sanitize_pnl_pct(0.0);
    assert!((result - 0.0).abs() < 1e-10);
}

#[test]
fn u1_4_sanitize_pnl_pct_negative() {
    let result = sanitize_pnl_pct(-0.25);
    assert!((result - (-0.25)).abs() < 1e-10);
}

#[test]
fn u2_1_derive_open_side_buy() {
    let result = derive_open_side("buy");
    assert_eq!(result, "sell");
}

#[test]
fn u2_2_derive_open_side_sell() {
    let result = derive_open_side("sell");
    assert_eq!(result, "buy");
}
