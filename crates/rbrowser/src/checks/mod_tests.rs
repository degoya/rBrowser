use std::sync::Arc;
use std::time::{Duration, Instant};

use rb_engine::{ViewHost, ViewId};
use rb_ipc::IpcErrorCode;
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::{RecordingUi, UiCall};

fn setup() -> (Arc<RecordingUi>, Arc<Core>) {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    let core = Core::new(host, ui.clone(), Handle::current(), CoreSetup::default());
    (ui, core)
}

fn findings_events(ui: &RecordingUi) -> Vec<String> {
    ui.calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::Shell(script) if script.contains(r#""type":"findings""#) => Some(script),
            _ => None,
        })
        .collect()
}

fn ask(core: &Core, message: Value) -> Result<CoreToShell, crate::ipc::Failure> {
    let message: ShellToCore = serde_json::from_value(message).expect("a shell message");
    ipc(core, &message).expect("a message of this module")
}

#[tokio::test]
async fn checks_run_all_answers_the_run_and_pushes_it_to_the_panel() {
    let (ui, core) = setup();
    let answer = run_command(&core, RUN_ALL_COMMAND, &Value::Null)
        .expect("this module's command")
        .expect("runs without any check plugin");
    assert!(
        answer["runId"]
            .as_str()
            .is_some_and(rb_plugin_host::valid_run_id),
        "{answer}"
    );
    assert_eq!(answer["target"], "all");
    assert_eq!(answer["findings"], json!([]));
    assert_eq!(answer["summary"]["total"], 0);
    assert_eq!(findings_events(&ui).len(), 1);
}

#[tokio::test]
async fn checks_run_all_refuses_bad_arguments_and_unknown_views() {
    let (_ui, core) = setup();
    for args in [
        json!({ "target": 99 }),
        json!({ "url": "javascript:alert(1)" }),
        json!({ "plugins": ["Not An Id"] }),
        json!({ "unknown": true }),
    ] {
        let result = run_command(&core, RUN_ALL_COMMAND, &args).expect("this module's command");
        assert!(
            matches!(result, Err(CommandError::InvalidArgs(_))),
            "{args}: {result:?}"
        );
    }
    assert!(run_command(&core, "view.back", &Value::Null).is_none());
}

#[tokio::test]
async fn one_run_at_a_time() {
    let first = Running::start().expect("nothing runs");
    assert!(Running::start().is_none());
    drop(first);
    assert!(Running::start().is_some());
}

#[tokio::test]
async fn run_checks_answers_at_once_and_the_run_follows() {
    let (ui, core) = setup();
    assert_eq!(
        ask(&core, json!({ "type": "runChecks", "target": "all" })),
        Ok(CoreToShell::Ack)
    );
    let deadline = Instant::now() + Duration::from_secs(20);
    while findings_events(&ui).is_empty() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(findings_events(&ui).len(), 1);
}

#[tokio::test]
async fn run_checks_refuses_what_checks_run_refuses() {
    let (_ui, core) = setup();
    let unknown = ask(&core, json!({ "type": "runChecks", "target": 99 }));
    assert_eq!(
        unknown.map_err(|failure| failure.0),
        Err(IpcErrorCode::NotFound)
    );
    let bad_url = ask(
        &core,
        json!({ "type": "runChecks", "target": "all", "url": "file:///etc/passwd" }),
    );
    assert_eq!(
        bad_url.map_err(|failure| failure.0),
        Err(IpcErrorCode::BadRequest)
    );
}

#[tokio::test]
async fn highlight_finding_checks_the_selector_and_the_views() {
    let (_ui, core) = setup();
    let view: ViewId = core.host().views()[0].id;
    let request = |selector: String, views: Vec<ViewId>| json!({ "type": "highlightFinding", "findingId": "9f2c4e1a", "selector": selector, "views": views });
    assert_eq!(
        ask(&core, request("img.hero".to_owned(), vec![view])),
        Ok(CoreToShell::Ack)
    );
    for refused in [
        request("a".repeat(highlight::MAX_SELECTOR_CHARS + 1), vec![view]),
        request("img\n".to_owned(), vec![view]),
        request("img".to_owned(), vec![ViewId(999)]),
        request("img".to_owned(), Vec::new()),
    ] {
        assert_eq!(
            ask(&core, refused.clone()).map_err(|failure| failure.0),
            Err(IpcErrorCode::BadRequest),
            "{refused}"
        );
    }
    assert!(ipc(&core, &ShellToCore::GetAbout).is_none());
}

/// RB-080-69: Findings opened a window of its own (`control/handlers.rs`, `panels.findings` ->
/// `PageWindow::Findings`); it is an entry of the panels page now, floating or docked like the
/// plugin panels, selected there when the page is open already.
#[tokio::test]
async fn panels_findings_opens_the_panels_page_at_findings() {
    let (ui, core) = setup();
    crate::panel_dock::tests::float(&core);
    core.run_command("panels.findings", Value::Null)
        .expect("the panels open");
    let url = "rbrowser://ui/settings.html#/panels/findings";
    assert!(
        ui.calls().contains(&UiCall::PageWindow(
            crate::page_window::PageWindow::Panels,
            url.to_owned()
        )),
        "{:?}",
        ui.calls()
    );
    let selects = ui.calls().into_iter().any(|call| {
        matches!(call, UiCall::Shell(script) if script.contains(r#""select":{"core":"findings"}"#))
    });
    assert!(selects, "an open page selects Findings");
    // The shell strip keeps its 44 px: no panel there.
    let in_shell = ui.calls().into_iter().any(|call| {
        matches!(call, UiCall::Shell(script) if script.contains(r#""id":"panels.findings""#))
    });
    assert!(!in_shell);
}

/// The findings window may open after the run: it asks for the last one on load.
#[tokio::test]
async fn the_findings_window_gets_the_last_run_when_it_opens() {
    let (_ui, core) = setup();
    let before = ask(&core, json!({ "type": "getFindings" })).expect("answered");
    assert_eq!(before, CoreToShell::Ack, "no run yet");
    let answer = run_command(&core, RUN_ALL_COMMAND, &Value::Null)
        .expect("this module's command")
        .expect("runs");
    let last = ask(&core, json!({ "type": "getFindings" })).expect("answered");
    let CoreToShell::Findings { run, .. } = last else {
        panic!("the last run: {last:?}");
    };
    assert_eq!(run["runId"], answer["runId"]);
}

/// `visual.compare`'s run is a check run like any other: one at a time (review c20, 2).
#[tokio::test(flavor = "multi_thread")]
async fn a_requested_run_waits_for_no_one_and_is_refused_while_another_runs() {
    let (_ui, core) = setup();
    let running = Running::start().expect("no run yet");
    let request = run::request(ChecksRunArgs::default(), Vec::new(), SystemTime::now());
    let refused = tokio::task::block_in_place(|| run_request(&core, request));
    assert!(
        matches!(refused, Err(RequestError::Busy)),
        "{:?}",
        refused.map(|file| file.run_id)
    );
    drop(running);
}
