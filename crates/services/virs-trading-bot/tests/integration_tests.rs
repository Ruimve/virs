use virs_trading_bot::{ChatAction, ChatDecision};
use virs_trading_bot::{
    compute_cooldown_secs, compute_position_pct, compute_stop_loss, compute_take_profit,
    compute_trailing_stop, format_stop_take_profit,
};

#[test]
fn int_1_1_stop_loss_take_profit_consistency() {
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
    let entry = 100.0;
    let atr = 2.0;
    let initial_stop = compute_stop_loss(entry, "long", atr);

    let new_stop_1 = compute_trailing_stop(entry, 105.0, "long", atr, initial_stop);
    assert!(
        new_stop_1 >= initial_stop,
        "trailing stop should never decrease"
    );

    let new_stop_2 = compute_trailing_stop(entry, 103.0, "long", atr, new_stop_1);
    assert!(
        new_stop_2 >= new_stop_1,
        "trailing stop should never worsen"
    );
}

#[test]
fn int_1_3_position_pct_full_chain() {
    let pct = compute_position_pct(30.0, 2, 0.003);
    assert_eq!(pct, 20.0);

    let pct_after = compute_position_pct(30.0, 0, 0.0);
    assert_eq!(pct_after, 80.0);
}

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
        let action = ChatAction::from_str(action_str);

        if action_str == "unknown_action" {
            assert_eq!(action, ChatAction::Hold);
            assert_eq!(action.as_str(), "hold");
        } else {
            assert_eq!(action.as_str(), action_str);
        }
    }
}

#[test]
fn int_2_2_chat_decision_json_roundtrip() {
    let json = serde_json::json!({
        "decision": {
            "action": "open_long",
            "reason": "Bullish divergence",
            "confidence": 0.9
        },
        "market": {
            "market_regime": "trending_up"
        }
    });

    let decision = ChatDecision::from_json(&json).expect("should parse");
    assert_eq!(decision.action, ChatAction::OpenLong);

    assert_eq!(decision.action.as_str(), "open_long");
}

#[test]
fn int_4_2_format_stop_take_with_position_pct() {
    let pct = compute_position_pct(25.0, 0, 0.0);
    assert_eq!(pct, 80.0);

    let sl = compute_stop_loss(100.0, "long", 2.0);
    let tp = compute_take_profit(100.0, "long", 2.0);
    let display = format_stop_take_profit(sl, tp);

    assert!(display.contains("止损"));
    assert!(display.contains("止盈"));
}

#[test]
fn int_5_1_cooldown_then_position_pct() {
    let cooldown = compute_cooldown_secs("long", "stop_loss", "long");
    assert_eq!(cooldown, 1800);

    let pct = compute_position_pct(25.0, 0, 0.0);
    assert_eq!(pct, 80.0);

    let pct_after_loss = compute_position_pct(25.0, 2, 0.0);
    assert_eq!(pct_after_loss, 40.0);
}

