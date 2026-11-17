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
