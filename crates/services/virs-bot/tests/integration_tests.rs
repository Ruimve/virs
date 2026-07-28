use uuid::Uuid;
use virs_bot::auto::ai::{AutoAction, AutoDecision};
use virs_bot::auto::strategy::{
    compute_cooldown_secs, compute_position_pct, compute_stop_loss, compute_take_profit,
    compute_trailing_stop, format_stop_take_profit,
};
use virs_bot::grid::ai::{parse_grid_decision, GridAction};
use virs_bot::grid::types::GridLevel;
use virs_bot::grid::utils::calculate_levels;
use virs_strategy::prompt::render::format_bars_outside;
use virs_types::grid_port::GridBotConfig;

fn make_bot(upper: f64, lower: f64, count: i32) -> GridBotConfig {
    GridBotConfig {
        id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        name: "test".to_string(),
        symbol: "BTC/USDT".to_string(),
        exchange: "binance".to_string(),
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
        strategy_file: None,
    }
}

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
        let action = AutoAction::from_str(action_str);

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
            "confidence": 0.9
        },
        "market": {
            "market_regime": "trending_up"
        }
    });

    let decision = AutoDecision::from_json(&json).expect("should parse");
    assert_eq!(decision.action, AutoAction::OpenLong);

    assert_eq!(decision.action.as_str(), "open_long");
}

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
            "quantity_per_grid": 50.0
        },
        "market": {
            "market_regime": "trending"
        },
        "analysis": "Volatility expanding",
        "risk_warning": "Watch for false breakouts"
    });

    let decision = parse_grid_decision(&json).expect("JSON should parse");
    assert_eq!(decision.action, "adjust_grid");
    assert!((decision.upper_price - 110.0).abs() < 1e-10);
    assert!((decision.lower_price - 90.0).abs() < 1e-10);

    let bot = make_bot(
        decision.upper_price,
        decision.lower_price,
        decision.grid_count,
    );
    let levels = calculate_levels(&bot, 100.0);
    assert_eq!(levels.len(), 10);

    for level in &levels {
        assert!(level.side == "buy" || level.side == "sell");
        assert!(level.sell_price > level.buy_price);
    }
}

#[test]
fn int_4_1_calculate_levels_then_reset() {
    let bot = make_bot(110.0, 90.0, 5);
    let levels = calculate_levels(&bot, 100.0);
    assert_eq!(levels.len(), 5);

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

    let reset: Vec<GridLevel> = traded.iter().map(|l| l.reset_for_relist()).collect();

    for level in &reset {
        assert!(!level.buy_filled);
        assert!((level.hold_quantity - 0.0).abs() < 1e-10);
        assert!(level.buy_order_id.is_none());

        assert!(level.sell_price > level.buy_price);
    }
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

#[test]
fn int_6_1_format_bars_outside_all_cases() {
    assert_eq!(format_bars_outside(5), "向上5根");
    assert_eq!(format_bars_outside(-3), "向下3根");
    assert_eq!(format_bars_outside(0), "无");
}
