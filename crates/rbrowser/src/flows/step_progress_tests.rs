use serde_json::json;

use super::*;

const RUN: &str = "20261118-185842-a1b2c3";

#[test]
fn a_step_of_the_progress_becomes_the_shells_flow_step() {
    let progress = json!({ "runId": RUN, "viewId": 2,
        "step": { "name": "Step 3: click", "id": 3, "state": "failed",
                  "message": "locator.click: sent Bearer abc.def now" } });
    assert_eq!(
        event(RUN, "checkout", &progress),
        Some(FlowStep {
            run_id: RUN.to_owned(),
            flow: "checkout".to_owned(),
            view: 2,
            name: "Step 3: click".to_owned(),
            step_id: Some(3),
            state: "failed".to_owned(),
            message: Some("locator.click: sent Bearer *** now".to_owned()),
        })
    );
    let plain = json!({ "viewId": 1, "step": { "name": "log in", "state": "running" } });
    let step = event(RUN, "login", &plain).expect("a step");
    assert_eq!(step.step_id, None);
    assert_eq!(step.message, None);
}

#[test]
fn a_status_or_a_broken_step_is_no_flow_step() {
    for progress in [
        json!({ "viewId": 1, "status": "running" }),
        json!({ "viewId": 1, "step": { "name": "x", "state": "done" } }),
        json!({ "viewId": "1", "step": { "name": "x", "state": "running" } }),
        json!({ "step": { "name": "x", "state": "running" } }),
        json!({ "viewId": 1, "step": { "state": "running" } }),
        json!({ "viewId": 1, "step": "Step 3" }),
    ] {
        assert_eq!(event(RUN, "login", &progress), None, "{progress}");
    }
}

#[test]
fn a_long_message_is_cut() {
    let long = "x".repeat(MAX_MESSAGE + 10);
    let progress = json!({ "viewId": 1,
        "step": { "name": "Step 1: fill", "state": "failed", "message": long } });
    let step = event(RUN, "login", &progress).expect("a step");
    assert_eq!(step.message.map(|m| m.chars().count()), Some(MAX_MESSAGE));
}
