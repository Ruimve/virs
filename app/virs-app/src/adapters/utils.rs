//! Shared utility functions for adapters.

/// Sanitize PnL percentage: replace NaN with 0.0.
pub fn sanitize_pnl_pct(pnl_pct: f64) -> f64 {
    if pnl_pct.is_nan() {
        0.0
    } else {
        pnl_pct
    }
}

/// Derive the open side from a close side.
/// "buy" → "sell", anything else → "buy".
pub fn derive_open_side(close_side: &str) -> &str {
    if close_side == "buy" {
        "sell"
    } else {
        "buy"
    }
}
