use serde_json::json;

use super::*;

fn view(id: u32, status: ViewStatus) -> ViewResult {
    ViewResult {
        view_id: id,
        label: format!("V{id}"),
        breakpoint: None,
        engine: "chromium".to_owned(),
        status,
        duration_ms: 10,
        steps: Vec::new(),
        error: None,
        screenshot: None,
        reason: None,
    }
}

fn run(views: Vec<ViewResult>) -> RunResult {
    RunResult {
        version: RESULT_VERSION,
        run_id: "20261111-101500-a1b2c3".to_owned(),
        flow: "login".to_owned(),
        file: ".rbrowser/flows/login.ts".to_owned(),
        started_at: "2026-11-11T10:15:00Z".to_owned(),
        duration_ms: 20,
        passed: !RunResult::failed(&views),
        views,
        skipped_breakpoints: Vec::new(),
        error: None,
    }
}

#[test]
fn the_sidecar_view_reads_with_its_camel_case_fields() {
    let view: ViewResult = serde_json::from_value(json!({
        "viewId": 2, "label": "Tablet", "breakpoint": "tablet", "engine": "chromium",
        "status": "failed", "durationMs": 120,
        "steps": [{ "name": "open", "status": "failed", "durationMs": 100 }],
        "error": { "message": "expect failed", "stack": "at a.ts:3:5", "step": "open",
                   "line": 3, "column": 5, "file": "/home/u/site/.rbrowser/flows/a.ts" },
        "screenshot": "tablet-2.png"
    }))
    .expect("reads");
    assert_eq!(view.status, ViewStatus::Failed);
    assert_eq!(view.error.as_ref().and_then(|error| error.line), Some(3));
    assert_eq!(view.screenshot.as_deref(), Some("tablet-2.png"));
    let timed: ViewStatus = serde_json::from_value(json!("timedOut")).expect("reads");
    assert_eq!(timed, ViewStatus::TimedOut);
}

#[test]
fn a_run_passes_unless_a_view_failed_or_timed_out() {
    let skipped = run(vec![
        view(1, ViewStatus::Passed),
        view(2, ViewStatus::Skipped),
    ]);
    assert!(skipped.passed);
    assert_eq!(skipped.counts(), (1, 0, 1));
    let timed = run(vec![
        view(1, ViewStatus::Passed),
        view(2, ViewStatus::TimedOut),
    ]);
    assert!(!timed.passed);
    assert_eq!(timed.counts(), (1, 1, 0));
}

#[test]
fn run_ids_are_time_and_random_and_name_a_folder() {
    let id = run_id("2026-11-11T10:15:00Z", "a1b2c3");
    assert_eq!(id, "20261111-101500-a1b2c3");
    assert!(valid_run_id(&id));
    for bad in ["", "../x", "a/b", "-x", "a b", &"a".repeat(65)] {
        assert!(!valid_run_id(bad), "{bad}");
    }
}

#[test]
fn the_result_is_written_whole_and_read_back() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let folder = run_dir(dir.path(), "20261111-101500-a1b2c3");
    assert_eq!(read(&folder), None);
    let result = run(vec![view(1, ViewStatus::Passed)]);
    let path = write(&folder, &result).expect("writes");
    assert!(path.ends_with(".rbrowser/out/flows/20261111-101500-a1b2c3/result.json"));
    assert!(!folder.join("result.json.partial").exists());
    assert_eq!(read(&folder), Some(result));
}

#[test]
fn the_run_folder_is_joined_part_by_part_so_windows_sees_no_forward_slash() {
    let folder = run_dir(Path::new("root"), "20261117-122216-f81d91");
    let parts: Vec<_> = folder
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        parts,
        [
            "root",
            ".rbrowser",
            "out",
            "flows",
            "20261117-122216-f81d91"
        ]
    );
}
