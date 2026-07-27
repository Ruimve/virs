use crate::output::{StrategyAction, StrategyOutput};
use uuid::Uuid;

#[test]
fn o1_action_as_str_roundtrip() {
    assert_eq!(StrategyAction::OpenLong.as_str(), "open_long");
    assert_eq!(StrategyAction::Hold.as_str(), "hold");
    assert_eq!(
        StrategyAction::AdjustGrid {
            upper_price: 100.0,
            lower_price: 90.0,
            grid_count: 10,
            grid_profit_pct: 0.5,
            quantity_per_grid: 1.0,
        }
        .as_str(),
        "adjust_grid"
    );
}

#[test]
fn o2_action_predicates() {
    assert!(StrategyAction::OpenLong.is_open_position());
    assert!(StrategyAction::OpenShort.is_open_position());
    assert!(!StrategyAction::ClosePosition.is_open_position());
    assert!(!StrategyAction::Hold.is_open_position());

    assert!(StrategyAction::Hold.is_noop());
    assert!(!StrategyAction::OpenLong.is_noop());

    assert!(StrategyAction::PauseGrid.is_grid_restructure());
    assert!(StrategyAction::RunGrid.is_grid_restructure());
    assert!(StrategyAction::ReducePosition.is_grid_restructure());
    assert!(StrategyAction::AdjustGrid {
        upper_price: 1.0,
        lower_price: 0.5,
        grid_count: 5,
        grid_profit_pct: 0.1,
        quantity_per_grid: 0.1
    }
    .is_grid_restructure());
    assert!(!StrategyAction::OpenLong.is_grid_restructure());
}

#[test]
fn o3_hold_factory() {
    let out = StrategyOutput::hold(Some(Uuid::nil()), "LLM unavailable");
    assert!(out.is_noop());
    assert_eq!(out.reason, "LLM unavailable");
    assert_eq!(out.confidence, 0.0);
    assert!(out.bot_id.is_some());
}
