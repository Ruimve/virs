/**
 * 测试 ai::AutoAction 枚举的 from_str 和 as_str
 * - "open_long" → OpenLong
 * - "open_short" → OpenShort
 * - "close_position" → ClosePosition
 * - "hold" → Hold
 * - 未知字符串 → Hold (fallback)
 * - 大小写不敏感
 */
use crate::bot::auto_trade::ai::AutoAction;

#[test]
fn open_long_from_str() {
    assert_eq!(AutoAction::from_str("open_long"), AutoAction::OpenLong);
}

#[test]
fn open_short_from_str() {
    assert_eq!(AutoAction::from_str("open_short"), AutoAction::OpenShort);
}

#[test]
fn close_position_from_str() {
    assert_eq!(AutoAction::from_str("close_position"), AutoAction::ClosePosition);
}

#[test]
fn hold_from_str() {
    assert_eq!(AutoAction::from_str("hold"), AutoAction::Hold);
}

#[test]
fn unknown_falls_back_to_hold() {
    assert_eq!(AutoAction::from_str("buy"), AutoAction::Hold);
    assert_eq!(AutoAction::from_str("sell"), AutoAction::Hold);
    assert_eq!(AutoAction::from_str(""), AutoAction::Hold);
    assert_eq!(AutoAction::from_str("OPEN_LONG"), AutoAction::OpenLong);
}

#[test]
fn as_str_roundtrip() {
    assert_eq!(AutoAction::OpenLong.as_str(), "open_long");
    assert_eq!(AutoAction::OpenShort.as_str(), "open_short");
    assert_eq!(AutoAction::ClosePosition.as_str(), "close_position");
    assert_eq!(AutoAction::Hold.as_str(), "hold");
}

#[test]
fn from_str_as_str_identity() {
    for action in [AutoAction::OpenLong, AutoAction::OpenShort, AutoAction::ClosePosition, AutoAction::Hold] {
        assert_eq!(AutoAction::from_str(action.as_str()), action);
    }
}
