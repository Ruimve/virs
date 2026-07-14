pub fn sanitize_pnl_pct(pnl_pct: f64) -> f64 {
    if pnl_pct.is_nan() {
        0.0
    } else {
        pnl_pct
    }
}


pub fn derive_open_side(close_side: &str) -> &str {
    if close_side == "buy" {
        "sell"
    } else {
        "buy"
    }
}
