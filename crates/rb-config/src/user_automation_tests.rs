use serde_json::json;

use super::*;

#[test]
fn defaults_run_every_view_for_thirty_seconds_with_screenshots_and_without_sync() {
    let settings = FlowSettings::default();
    assert_eq!(settings.dir, ".rbrowser/flows");
    assert_eq!(settings.parallel, 0);
    assert_eq!(settings.timeout_ms, 30_000);
    // One action waits a third of that, so a click on a hidden element fails with its locator
    // before the flow's time is up (RB-090-76).
    assert_eq!(settings.action_timeout_ms, 10_000);
    assert!(!settings.sync_during_run);
    assert!(settings.screenshot_on_failure);
    // RB-090-84, the owner's decision: a run skips the views the layout hides by default.
    assert_eq!(settings.hidden_views, HiddenViews::Skip);
    assert_eq!(settings.run_layout, "grid");
    // RB-090-89: a played recording whose views are all hidden comes into sight by default.
    assert!(settings.reveal_recording);
}

#[test]
fn bringing_a_recording_into_sight_can_be_switched_off() {
    let settings: AutomationSettings =
        serde_json::from_value(json!({ "flows": { "revealRecording": false } })).expect("reads");
    assert!(!settings.flows.reveal_recording);
}

#[test]
fn deleting_a_flow_asks_first_by_default_and_can_be_switched_off() {
    // RB-090-91: "Delete" in the Flows panel asks in the panel before the file goes.
    assert!(FlowSettings::default().confirm_delete);
    let settings: AutomationSettings =
        serde_json::from_value(json!({ "flows": { "confirmDelete": false } })).expect("reads");
    assert!(!settings.flows.confirm_delete);
}

#[test]
fn a_run_can_show_a_layout_of_its_own() {
    let settings: AutomationSettings = serde_json::from_value(json!({
        "flows": { "hiddenViews": "layout", "runLayout": "horizontal" }
    }))
    .expect("reads");
    assert_eq!(settings.flows.hidden_views, HiddenViews::Layout);
    assert_eq!(settings.flows.run_layout, "horizontal");
    assert!(
        serde_json::from_value::<AutomationSettings>(json!({ "flows": { "hiddenViews": "show" } }))
            .is_err()
    );
    assert_eq!(RUN_LAYOUTS, ["grid", "horizontal", "split", "canvas"]);
}

#[test]
fn keys_read_in_camel_case_and_missing_ones_default() {
    let settings: AutomationSettings = serde_json::from_value(json!({
        "flows": { "timeoutMs": 5000, "actionTimeoutMs": 2000, "syncDuringRun": true }
    }))
    .expect("reads");
    assert_eq!(settings.flows.timeout_ms, 5000);
    assert_eq!(settings.flows.action_timeout_ms, 2000);
    assert!(settings.flows.sync_during_run);
    assert_eq!(settings.flows.dir, DEFAULT_FLOWS_DIR);
}
