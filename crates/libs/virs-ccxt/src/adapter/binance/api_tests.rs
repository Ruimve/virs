//! Unit tests for adapter/binance/api.rs functions.
//!
//! Covers: decimal_places precision derivation helper.

use crate::adapter::binance::api::decimal_places;

#[test]
fn t_decimal_places_basic() {
    assert_eq!(decimal_places("0.01000000"), Some(2));
    assert_eq!(decimal_places("0.00100000"), Some(3));
    assert_eq!(decimal_places("0.00010000"), Some(4));
    assert_eq!(decimal_places("1.00000000"), Some(0));
    assert_eq!(decimal_places("0.1"), Some(1));
    assert_eq!(decimal_places("1"), None); // no decimal point
}

#[test]
fn t_decimal_places_edge() {
    assert_eq!(decimal_places("0.00001000"), Some(5));
    assert_eq!(decimal_places("10.50000000"), Some(1)); // 10.5 → 1 decimal place
}
