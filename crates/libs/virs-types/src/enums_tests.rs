use crate::enums::*;


#[test]
fn e11_1_open_is_open() {
    assert!(PositionStatus::Open.is_open());
    assert!(!PositionStatus::Closed.is_open());
}


#[test]
fn e14_1_running_is_running() {
    assert!(EngineState::Running.is_running());
    assert!(!EngineState::Stopped.is_running());
}
