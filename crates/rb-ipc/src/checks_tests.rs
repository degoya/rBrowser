use serde_json::json;

use super::{CheckJob, CheckJobState, ChecksProgress};
use crate::{CoreToShell, ShellToCore, ViewId, parse_request};

fn progress() -> ChecksProgress {
    ChecksProgress {
        run_id: "run-0000000000001".to_owned(),
        elapsed_ms: 1500,
        estimate_ms: Some(12_000),
        jobs: vec![
            CheckJob {
                plugin: "seo".to_owned(),
                view: Some(ViewId(1)),
                breakpoint: Some("mobile".to_owned()),
                state: CheckJobState::Done,
                findings: Some(3),
                reason: None,
                reason_params: Default::default(),
                elapsed_ms: Some(900),
            },
            CheckJob {
                plugin: "a11y-axe".to_owned(),
                view: Some(ViewId(1)),
                breakpoint: Some("mobile".to_owned()),
                state: CheckJobState::Failed,
                findings: None,
                reason: Some("plugins.error.not_available".to_owned()),
                reason_params: [("service".to_owned(), "evaluate".to_owned())].into(),
                elapsed_ms: None,
            },
        ],
        finished: false,
        cancelled: false,
    }
}

#[test]
fn the_progress_event_shape() {
    let message = CoreToShell::ChecksProgress(progress());
    assert_eq!(
        serde_json::to_value(&message).expect("json"),
        json!({
            "type": "checksProgress",
            "runId": "run-0000000000001",
            "elapsedMs": 1500,
            "estimateMs": 12000,
            "jobs": [
                { "plugin": "seo", "view": 1, "breakpoint": "mobile", "state": "done",
                  "findings": 3, "elapsedMs": 900 },
                { "plugin": "a11y-axe", "view": 1, "breakpoint": "mobile", "state": "failed",
                  "reason": "plugins.error.not_available",
                  "reasonParams": { "service": "evaluate" } }
            ],
            "finished": false
        })
    );
    assert_eq!(progress().counts(), (2, 2));
}

#[test]
fn the_progress_is_asked_for_and_a_partial_run_is_marked() {
    assert_eq!(
        parse_request(r#"{ "type": "getChecksProgress" }"#).expect("parses"),
        ShellToCore::GetChecksProgress
    );
    let partial = CoreToShell::Findings {
        run: json!({}),
        messages: Default::default(),
        project_off: Default::default(),
        partial: true,
    };
    assert_eq!(
        serde_json::to_value(&partial).expect("json"),
        json!({ "type": "findings", "run": {}, "messages": {}, "partial": true })
    );
}
