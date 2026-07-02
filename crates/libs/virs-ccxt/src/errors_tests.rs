//! Unit tests for errors.rs.
//!
//! Covers: ExchangeError::no_data.

use virs_error::ExchangeError;

// ============================================================
// TC-E2: ExchangeError::no_data
// ============================================================

#[test]
fn e2_1_no_data_construction() {
    let err = ExchangeError::no_data("No ticker found for BTC/USDT".to_string());
    match err {
        ExchangeError::NoData(msg) => {
            assert_eq!(msg, "No ticker found for BTC/USDT");
        }
        _ => panic!("Expected NoData variant"),
    }
}
