use crate::auto::strategy::*;

#[test]
fn s1_1_stop_loss_long() {
    let result = compute_stop_loss(100.0, "long", 2.0);
    assert!((result - 97.0).abs() < 1e-10);
}

#[test]
fn s1_2_stop_loss_short() {
    let result = compute_stop_loss(100.0, "short", 2.0);
    assert!((result - 103.0).abs() < 1e-10);
}

#[test]
fn s1_3_stop_loss_zero_atr() {
    let result = compute_stop_loss(100.0, "long", 0.0);
    assert!((result - 97.0).abs() < 1e-10);
}

#[test]
fn s1_4_stop_loss_zero_entry() {
    let result = compute_stop_loss(0.0, "long", 2.0);
    assert!((result - 0.0).abs() < 1e-10);
}

#[test]
fn s1_5_take_profit_long() {
    let result = compute_take_profit(100.0, "long", 2.0);
    assert!((result - 106.0).abs() < 1e-10);
}

#[test]
fn s1_6_take_profit_short() {
    let result = compute_take_profit(100.0, "short", 2.0);
    assert!((result - 94.0).abs() < 1e-10);
}

#[test]
fn s1_7_take_profit_zero_atr() {
    let result = compute_take_profit(100.0, "long", 0.0);
    assert!((result - 106.0).abs() < 1e-10);
}

#[test]
fn s2_1_trailing_long_profit_2atr() {
    let result = compute_trailing_stop(100.0, 105.0, "long", 2.0, 97.0);
    assert!((result - 103.0).abs() < 1e-10);
}

#[test]
fn s2_2_trailing_long_profit_1atr() {
    let result = compute_trailing_stop(100.0, 102.0, "long", 2.0, 97.0);
    assert!((result - 100.0).abs() < 1e-10);
}

#[test]
fn s2_3_trailing_short_profit_2atr() {
    let result = compute_trailing_stop(100.0, 95.0, "short", 2.0, 103.0);
    assert!((result - 97.0).abs() < 1e-10);
}

#[test]
fn s2_4_trailing_no_profit() {
    let result = compute_trailing_stop(100.0, 101.0, "long", 2.0, 97.0);
    assert!((result - 97.0).abs() < 1e-10);
}

#[test]
fn s2_5_trailing_long_never_worsens() {
    let result = compute_trailing_stop(100.0, 105.0, "long", 2.0, 105.0);
    assert!((result - 105.0).abs() < 1e-10);
}

#[test]
fn s3_1_position_pct_high_adx() {
    let result = compute_position_pct(25.0, 0, 0.0);
    assert!((result - 80.0).abs() < 1e-10);
}

#[test]
fn s3_2_position_pct_medium_adx() {
    let result = compute_position_pct(20.0, 0, 0.0);
    assert!((result - 60.0).abs() < 1e-10);
}

#[test]
fn s3_3_position_pct_low_adx() {
    let result = compute_position_pct(15.0, 0, 0.0);
    assert!((result - 40.0).abs() < 1e-10);
}

#[test]
fn s3_4_position_pct_consecutive_losses() {
    let result = compute_position_pct(25.0, 2, 0.0);
    assert!((result - 40.0).abs() < 1e-10);
}

#[test]
fn s3_5_position_pct_high_funding() {
    let result = compute_position_pct(25.0, 0, 0.002);
    assert!((result - 40.0).abs() < 1e-10);
}

#[test]
fn s3_6_position_pct_clamp_min() {
    let result = compute_position_pct(15.0, 2, 0.002);
    assert!((result - 10.0).abs() < 1e-10);
}

#[test]
fn s4_1_format_stop_take_both() {
    let result = format_stop_take_profit(95.0, 110.0);
    assert!(result.contains("止损"));
    assert!(result.contains("止盈"));
}

#[test]
fn s4_2_format_stop_take_none() {
    let result = format_stop_take_profit(0.0, 0.0);
    assert_eq!(result, "未设置");
}

#[test]
fn s4_3_format_stop_take_only_sl() {
    let result = format_stop_take_profit(95.0, 0.0);
    assert!(result.contains("止损"));
    assert!(!result.contains("止盈"));
}

#[test]
fn s5_1_cooldown_stop_loss_same_side() {
    let result = compute_cooldown_secs("long", "stop_loss", "long");
    assert_eq!(result, 1800);
}

#[test]
fn s5_2_cooldown_stop_loss_diff_side() {
    let result = compute_cooldown_secs("long", "stop_loss", "short");
    assert_eq!(result, 0);
}

#[test]
fn s5_3_cooldown_take_profit_same_side() {
    let result = compute_cooldown_secs("long", "take_profit", "long");
    assert_eq!(result, 900);
}

#[test]
fn s5_4_cooldown_llm_decision() {
    let result = compute_cooldown_secs("long", "llm_decision", "long");
    assert_eq!(result, 900);
}

#[test]
fn s5_5_cooldown_unknown_reason() {
    let result = compute_cooldown_secs("long", "unknown", "short");
    assert_eq!(result, 900);
}
