//! Integration tests for virs-bot — cross-module chain verification.

use virs_bot::auto::ai::{AutoAction, AutoDecision};
use virs_bot::auto::strategy::{
    compute_cooldown_secs, compute_position_pct, compute_stop_loss, compute_take_profit,
    compute_trailing_stop, format_stop_take_profit,
};
use virs_bot::grid::ai::{parse_grid_decision, GridAction};
use virs_bot::grid::types::GridLevel;
use virs_bot::grid::utils::calculate_levels;
use virs_bot::grid::utils::prompt::format_bars_outside;
use uuid::Uuid;
use virs_types::grid_port::GridBotConfig;

// ── helpers ────────────────────────────────────────────────

fn make_bot(upper: f64, lower: f64, count: i32) -> GridBotConfig {
    GridBotConfig {
        id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        name: "test".to_string(),
        symbol: "BTC/USDT".to_string(),
        exchange: "binance".to_string(),
        market_type: "futures".to_string(),
        paper_mode: false,
        grid_count: count,
        upper_price: upper,
        lower_price: lower,
        grid_profit_pct: 0.5,
        quantity_per_grid: 100.0,
        leverage: 5,
        dynamic_adjust: false,
        adjust_interval_secs: 300,
        market_regime: None,
        grid_levels_json: None,
        system_prompt: None,
        last_adjusted_at: None,
    }
}

// ── INT-1: Strategy chain ──────────────────────────────────

#[test]
fn int_1_1_stop_loss_take_profit_consistency() {
    // For a long position: stop_loss < entry < take_profit
    let entry = 100.0;
    let atr = 2.0;
    let sl = compute_stop_loss(entry, "long", atr);
    let tp = compute_take_profit(entry, "long", atr);
    assert!(sl < entry, "stop_loss must be below entry for long");
    assert!(tp > entry, "take_profit must be above entry for long");
    assert!(sl < tp, "stop_loss must be below take_profit");
}

#[test]
fn int_1_2_trailing_stop_never_worsens() {
    // Trailing stop should only move in favorable direction
    let entry = 100.0;
    let atr = 2.0;
    let initial_stop = compute_stop_loss(entry, "long", atr);

    // Price moves up significantly
    let new_stop_1 = compute_trailing_stop(entry, 105.0, "long", atr, initial_stop);
    assert!(
        new_stop_1 >= initial_stop,
        "trailing stop should never decrease"
    );

    // Price moves up more — trailing should not worsen
    let new_stop_2 = compute_trailing_stop(entry, 103.0, "long", atr, new_stop_1);
    assert!(
        new_stop_2 >= new_stop_1,
        "trailing stop should never worsen"
    );
}

#[test]
fn int_1_3_position_pct_full_chain() {
    // Simulate the full risk management chain:
    // High ADX → base 80%, 2 consecutive losses → *0.5, high funding → *0.5
    let pct = compute_position_pct(30.0, 2, 0.003);
    assert_eq!(pct, 20.0); // 80 * 0.5 * 0.5 = 20

    // After cooldown, fewer losses → higher position
    let pct_after = compute_position_pct(30.0, 0, 0.0);
    assert_eq!(pct_after, 80.0);
}

// ── INT-2: Auto AI chain ───────────────────────────────────

#[test]
fn int_2_1_auto_action_roundtrip() {
    let actions = [
        "open_long",
        "open_short",
        "close_position",
        "hold",
        "unknown_action",
    ];
    for action_str in actions {
        let action = AutoAction::from_str(action_str);
        // from_str → as_str should be consistent for known actions
        if action_str == "unknown_action" {
            assert_eq!(action, AutoAction::Hold);
            assert_eq!(action.as_str(), "hold");
        } else {
            assert_eq!(action.as_str(), action_str);
        }
    }
}

#[test]
fn int_2_2_auto_decision_json_roundtrip() {
    let json = serde_json::json!({
        "decision": {
            "action": "open_long",
            "reason": "Bullish divergence",
            "confidence": 0.9,
            "stop_loss": 95000.0,
            "take_profit": 105000.0
        },
        "market": {
            "market_regime": "trending_up"
        }
    });

    let decision = AutoDecision::from_json(&json);
    assert_eq!(decision.action, AutoAction::OpenLong);

    // Verify the action can round-trip back to string
    assert_eq!(decision.action.as_str(), "open_long");

    // Stop loss and take profit should be consistent with strategy calculations
    if let (Some(sl), Some(tp)) = (decision.stop_loss, decision.take_profit) {
        let computed_sl = compute_stop_loss(100000.0, "long", 2000.0);
        let computed_tp = compute_take_profit(100000.0, "long", 2000.0);
        assert!(sl < 100000.0, "stop_loss below entry");
        assert!(tp > 100000.0, "take_profit above entry");
        // Computed values should also be consistent
        assert!(computed_sl < computed_tp);
    }
}

// ── INT-3: Grid AI chain ───────────────────────────────────

#[test]
fn int_3_1_grid_action_roundtrip() {
    let action = GridAction::from_str("adjust_grid", 100.0, 90.0);
    assert_eq!(action.as_str(), "adjust_grid");

    let action = GridAction::from_str("pause_grid", 0.0, 0.0);
    assert_eq!(action.as_str(), "pause_grid");

    let action = GridAction::from_str("unknown", 0.0, 0.0);
    assert_eq!(action.as_str(), "hold");
}

#[test]
fn int_3_2_grid_decision_parse_chain() {
    let json = serde_json::json!({
        "decision": {
            "action": "adjust_grid",
            "reason": "Volatility expansion",
            "confidence": 0.85
        },
        "grid": {
            "upper_price": 110.0,
            "lower_price": 90.0,
            "grid_count": 10,
            "grid_profit_pct": 0.5
        },
        "risk": {
            "leverage": 5,
            "quantity_per_grid": 50.0
        }
    });

    let decision = parse_grid_decision(&json).unwrap();
    assert_eq!(decision.action, "adjust_grid");
    assert!((decision.upper_price - 110.0).abs() < 1e-10);
    assert!((decision.lower_price - 90.0).abs() < 1e-10);

    // Use the parsed prices to create a bot config and calculate levels
    let bot = make_bot(decision.upper_price, decision.lower_price, decision.grid_count);
    let levels = calculate_levels(&bot, 100.0);
    assert_eq!(levels.len(), 10);

    // Verify all levels have valid buy/sell sides
    for level in &levels {
        assert!(level.side == "buy" || level.side == "sell");
        assert!(level.sell_price > level.buy_price);
    }
}

// ── INT-4: Grid lifecycle chain ────────────────────────────

#[test]
fn int_4_1_calculate_levels_then_reset() {
    let bot = make_bot(110.0, 90.0, 5);
    let levels = calculate_levels(&bot, 100.0);
    assert_eq!(levels.len(), 5);

    // Simulate trading: fill some orders
    let traded: Vec<GridLevel> = levels
        .iter()
        .map(|l| {
            let mut l = l.clone();
            l.buy_filled = true;
            l.hold_quantity = l.quantity;
            l.buy_order_id = Some(Uuid::new_v4());
            l
        })
        .collect();

    // Reset for relist
    let reset: Vec<GridLevel> = traded
        .iter()
        .map(|l| l.reset_for_relist())
        .collect();

    for level in &reset {
        assert!(!level.buy_filled);
        assert!((level.hold_quantity - 0.0).abs() < 1e-10);
        assert!(level.buy_order_id.is_none());
        // Config preserved
        assert!(level.sell_price > level.buy_price);
    }
}

#[test]
fn int_4_2_format_stop_take_with_position_pct() {
    // Simulate risk management: compute position, then format SL/TP for display
    let pct = compute_position_pct(25.0, 0, 0.0);
    assert_eq!(pct, 80.0);

    let sl = compute_stop_loss(100.0, "long", 2.0);
    let tp = compute_take_profit(100.0, "long", 2.0);
    let display = format_stop_take_profit(sl, tp);

    assert!(display.contains("止损"));
    assert!(display.contains("止盈"));
}

// ── INT-5: Cooldown + position chain ───────────────────────

#[test]
fn int_5_1_cooldown_then_position_pct() {
    // After a stop_loss on same side, cooldown applies
    let cooldown = compute_cooldown_secs("long", "stop_loss", "long");
    assert_eq!(cooldown, 1800); // 30 min

    // During cooldown, position would be 0% (no new trade)
    // After cooldown, position is computed normally
    let pct = compute_position_pct(25.0, 0, 0.0);
    assert_eq!(pct, 80.0);

    // After stop_loss with consecutive losses, position is reduced
    let pct_after_loss = compute_position_pct(25.0, 2, 0.0);
    assert_eq!(pct_after_loss, 40.0); // 80 * 0.5
}

// ── INT-6: format_bars_outside usage ───────────────────────

#[test]
fn int_6_1_format_bars_outside_all_cases() {
    assert_eq!(format_bars_outside(5), "向上5根");
    assert_eq!(format_bars_outside(-3), "向下3根");
    assert_eq!(format_bars_outside(0), "无");
}
