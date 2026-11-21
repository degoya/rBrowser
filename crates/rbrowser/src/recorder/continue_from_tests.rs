use serde_json::json;

use super::*;

fn result(value: Value) -> RunResult {
    serde_json::from_value(value).expect("a run result")
}

fn run(passed: bool, views: Value, error: Option<&str>) -> RunResult {
    let mut value = json!({
        "version": 1, "runId": "20261120-101500-a1b2c3", "flow": "search",
        "file": ".rbrowser/out/recordings/search.json", "startedAt": "2026-11-20T10:15:00Z",
        "durationMs": 1200, "passed": passed, "views": views
    });
    if let Some(error) = error {
        value["error"] = json!(error);
    }
    result(value)
}

#[test]
fn steps_that_passed_let_the_recording_start() {
    assert_eq!(failure(&run(true, json!([]), None)), None);
}

#[test]
fn a_failed_step_before_names_its_view_and_message() {
    let views = json!([
        { "viewId": 1, "label": "Mobile", "engine": "chromium", "status": "passed",
          "durationMs": 900, "steps": [] },
        { "viewId": 2, "label": "Desktop", "engine": "chromium", "status": "failed",
          "durationMs": 1100, "steps": [],
          "error": { "message": "Step 3: click - locator.click: Timeout", "stack": "", "file": "search.ts" } }
    ]);
    assert_eq!(
        failure(&run(false, views, None)).as_deref(),
        Some("Desktop: Step 3: click - locator.click: Timeout")
    );
}

#[test]
fn a_run_that_reached_no_view_names_why() {
    assert_eq!(
        failure(&run(false, json!([]), Some("no endpoint"))).as_deref(),
        Some("no endpoint")
    );
    assert_eq!(
        failure(&run(false, json!([]), None)).as_deref(),
        Some("a step before it failed")
    );
}

#[test]
fn the_notice_names_the_recording_and_masks_the_error() {
    let CoreToShell::Notification(notice) = failed_notice("search", "Desktop: timeout") else {
        panic!("a notification");
    };
    assert_eq!(notice.message_key, FAILED_NOTICE);
    assert_eq!(notice.level, NotificationLevel::Error);
    assert_eq!(notice.args.get("name").map(String::as_str), Some("search"));
    assert_eq!(
        notice.args.get("error").map(String::as_str),
        Some("Desktop: timeout")
    );
}

#[test]
fn the_arguments_name_the_recording_and_its_step() {
    let args: Args =
        serde_json::from_value(json!({ "recordingId": "search", "stepId": 3 })).expect("arguments");
    assert_eq!((args.recording_id.as_str(), args.step_id), ("search", 3));
    assert!(serde_json::from_value::<Args>(json!({ "recordingId": "search" })).is_err());
    assert!(
        serde_json::from_value::<Args>(json!({ "recordingId": "s", "stepId": 1, "x": 1 })).is_err()
    );
}

#[test]
fn the_new_steps_are_written_into_the_recording_they_continue() {
    let folder = tempfile::tempdir().expect("a folder");
    // A project has its `.rbrowser/`; the store refuses a folder outside one (a recording to
    // continue is always stored, so the real path never meets a project without it).
    std::fs::create_dir(folder.path().join(".rbrowser")).expect("the project's folder");
    let store =
        RecordingStore::open(folder.path(), ".rbrowser/out/recordings", &[]).expect("a store");
    let base: Recording = serde_json::from_value(json!({
        "version": 1, "name": "search", "startedAtMs": 0, "appVersion": "0.9.2",
        "steps": [
            { "id": 1, "action": { "type": "navigate", "url": "https://example.test/" }, "view": "mobile", "atMs": 0 },
            { "id": 2, "action": { "type": "wait", "ms": 5 }, "view": "mobile", "atMs": 800 }
        ]
    }))
    .expect("a recording");
    store.save(&base).expect("saved");
    let segment: Recording = serde_json::from_value(json!({
        "version": 1, "name": "search", "startedAtMs": 50, "appVersion": "0.9.2",
        "steps": [
            { "id": 1, "action": { "type": "wait", "ms": 9 }, "view": "mobile", "atMs": 200 }
        ]
    }))
    .expect("a segment");
    let continuing = Continuing {
        name: "search".to_owned(),
        after: 1,
    };
    let (whole, path) = continuing.save(&store, &segment).expect("saved");
    let ids: Vec<u32> = whole.steps.iter().map(|step| step.id).collect();
    assert_eq!(ids, [1, 3, 2]);
    assert_eq!(store.load("search").expect("read back"), whole);
    assert!(path.ends_with("search.json"));
}
