use super::round_number::{compute_up, compute_down};

#[test]
fn up_rounds_to_next_hundred() {

    let val = compute_up(4567.0);
    assert_eq!(val, 4600.0, "4567 rounded up should be 4600, got {val}");
}

#[test]
fn down_rounds_to_prev_hundred() {

    let val = compute_down(4567.0);
    assert_eq!(val, 4500.0, "4567 rounded down should be 4500, got {val}");
}

#[test]
fn up_rounds_small_price() {

    let val = compute_up(123.0);
    assert_eq!(val, 130.0, "123 rounded up should be 130, got {val}");
}

#[test]
fn down_rounds_small_price() {

    let val = compute_down(123.0);
    assert_eq!(val, 120.0, "123 rounded down should be 120, got {val}");
}

#[test]
fn already_at_round_number_stays() {
    assert_eq!(compute_up(100.0), 100.0, "100 rounded up should stay 100");
    assert_eq!(compute_down(100.0), 100.0, "100 rounded down should stay 100");
}

#[test]
fn zero_price_returns_zero() {
    assert_eq!(compute_up(0.0), 0.0);
    assert_eq!(compute_down(0.0), 0.0);
}

#[test]
fn large_price_uses_thousands_step() {

    let val = compute_up(56789.0);
    assert_eq!(val, 57000.0, "56789 rounded up should be 57000, got {val}");
}
