use super::*;
use crate::flows::result::{RESULT_VERSION, RunResult, ViewResult, ViewStatus, write};

fn last() -> RunResult {
    RunResult {
        version: RESULT_VERSION,
        run_id: "20261117-101500-a1b2c3".to_owned(),
        flow: "login".to_owned(),
        file: ".rbrowser/flows/login.ts".to_owned(),
        started_at: "2026-11-17T10:15:00Z".to_owned(),
        duration_ms: 20,
        passed: false,
        views: vec![ViewResult {
            view_id: 2,
            label: "Mobile".to_owned(),
            breakpoint: Some("mobile".to_owned()),
            engine: "chromium".to_owned(),
            status: ViewStatus::Failed,
            duration_ms: 10,
            steps: Vec::new(),
            error: None,
            screenshot: Some("mobile-2.png".to_owned()),
            reason: None,
        }],
        skipped_breakpoints: Vec::new(),
        error: None,
    }
}

#[test]
fn the_run_going_on_lists_its_views_statuses() {
    let running = FlowRunning {
        run_id: "run-1".to_owned(),
        flow: "login".to_owned(),
        views: vec![
            FlowViewStatus {
                view: 1,
                status: "running".to_owned(),
            },
            FlowViewStatus {
                view: 2,
                status: "passed".to_owned(),
            },
        ],
        started_at_ms: 1_794_910_500_000,
        queued: vec!["cart".to_owned()],
    };
    let state = state_of(true, Some(running), None);
    assert!(state.project);
    let running = state.running.expect("a run");
    assert_eq!(
        (running.run_id.as_str(), running.flow.as_str()),
        ("run-1", "login")
    );
    // The elapsed time and the flows of "Run all" still waiting (RB-090-71).
    assert_eq!(running.started_at_ms, 1_794_910_500_000);
    assert_eq!(running.queued, ["cart"]);
    assert_eq!(
        running.views,
        vec![
            FlowViewStatus {
                view: 1,
                status: "running".to_owned()
            },
            FlowViewStatus {
                view: 2,
                status: "passed".to_owned()
            },
        ]
    );
    assert_eq!(state.last, None);
}

#[test]
fn the_last_result_is_read_from_its_folder() {
    let dir = tempfile::tempdir().expect("temp dir");
    write(dir.path(), &last()).expect("written");
    let state = state_of(true, None, Some(dir.path()));
    assert_eq!(state.running, None);
    let last = state.last.expect("the last result");
    assert_eq!(last["flow"], "login");
    assert_eq!(last["views"][0]["status"], "failed");
    assert_eq!(last["views"][0]["screenshot"], "mobile-2.png");
    // A folder without a result: nothing.
    let empty = tempfile::tempdir().expect("temp dir");
    assert_eq!(state_of(false, None, Some(empty.path())).last, None);
}

#[test]
fn the_menu_knows_the_flows_of_the_project_it_listed() {
    let root = PathBuf::from("/work/shop");
    remember(root.clone(), vec!["login".to_owned()]);
    assert_eq!(known_for(&root), Some(vec!["login".to_owned()]));
    assert_eq!(known_for(Path::new("/work/other")), None);
}
