use serde_json::json;

use super::*;

#[test]
fn a_view_names_its_forced_states_by_cdps_words() {
    let forced = ViewForcedStates {
        view: ViewId(2),
        states: vec!["hover".into(), "focus-within".into()],
    };
    let json = serde_json::to_value(&forced).expect("json");
    assert_eq!(
        json,
        json!({ "view": 2, "states": ["hover", "focus-within"] })
    );
    let back: ViewForcedStates = serde_json::from_value(json).expect("reads back");
    assert_eq!(back, forced);
}

#[test]
fn the_forced_states_activity_is_written_forced_states() {
    assert_eq!(
        serde_json::to_value(crate::Activity::ForcedStates).expect("json"),
        json!("forcedStates")
    );
    let read: crate::Activity = serde_json::from_value(json!("forcedStates")).expect("reads");
    assert_eq!(read, crate::Activity::ForcedStates);
}
