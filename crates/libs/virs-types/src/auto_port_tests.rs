//! Unit tests for auto_port.rs AutoMarketType methods.

use crate::auto_port::AutoMarketType;

// ============================================================
// TC-AM2: AutoMarketType::is_spot
// ============================================================

#[test]
fn am2_1_spot_is_spot() {
    assert!(AutoMarketType::Spot.is_spot());
    assert!(!AutoMarketType::Perpetual.is_spot());
}
