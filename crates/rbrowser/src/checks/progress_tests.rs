use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rb_ipc::{CheckJobState, ShellToCore};
use rb_plugin_host::{Cancel, JobProgress, JobState, ProgressSink, RunOutcome, SkipReason};
use rb_plugin_types::FindingsFile;
use serde_json::json;
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

/// The shell events of `kind` (`checksProgress`, `findings`) the core sent, parsed.
fn events(ui: &RecordingUi, kind: &str) -> Vec<serde_json::Value> {
    ui.calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::Shell(script) if script.contains(&format!(r#""type":"{kind}""#)) => {
                let start = script.find("detail:")? + "detail:".len();
                let end = script.rfind("}));")?;
                serde_json::from_str(&script[start..end]).ok()
            }
            _ => None,
        })
        .collect()
}

fn job(plugin: &str, view: u32, state: JobState) -> JobProgress {
    JobProgress {
        plugin: plugin.to_owned(),
        view: Some(view),
        state,
        elapsed_ms: None,
    }
}

fn sink(core: Arc<Core>) -> Arc<PanelSink> {
    PanelSink::start(
        core,
        "run-0000000000001".to_owned(),
        BTreeMap::from([(1, "mobile".to_owned())]),
        Some(4000),
        Default::default(),
        Arc::new(Cancel::default()),
    )
}

fn empty_file() -> FindingsFile {
    serde_json::from_value(json!({
        "version": 1, "runId": "run-0000000000001", "startedAt": 1, "target": "all",
        "plugins": ["seo"], "views": [], "findings": [],
        "summary": { "total": 0, "error": 0, "warning": 0, "info": 0, "byPlugin": {} }
    }))
    .expect("a findings file")
}

#[test]
fn the_throttle_sends_the_first_change_and_one_late_send_for_the_rest() {
    let mut throttle = Throttle::default();
    let start = Instant::now();
    assert_eq!(throttle.offer(start), Offer::Now);
    assert_eq!(
        throttle.offer(start + Duration::from_millis(100)),
        Offer::Later(Duration::from_millis(150))
    );
    assert_eq!(
        throttle.offer(start + Duration::from_millis(120)),
        Offer::Skip,
        "the late send takes it along"
    );
    throttle.sent(start + Duration::from_millis(250));
    assert_eq!(
        throttle.offer(start + Duration::from_millis(600)),
        Offer::Now
    );
}

#[test]
fn the_jobs_go_to_the_panel_with_their_breakpoints_and_reasons() {
    let jobs = [
        job("seo", 1, JobState::Done { findings: 2 }),
        job(
            "a11y-axe",
            1,
            JobState::ended_by(&rb_plugin_host::not_available("sidecar"), Some(1)),
        ),
        job("lighthouse", 2, JobState::skipped(SkipReason::Cancelled)),
    ];
    let shown = ipc_jobs(&jobs, &BTreeMap::from([(1, "mobile".to_owned())]));
    assert_eq!(shown[0].state, CheckJobState::Done);
    assert_eq!(shown[0].findings, Some(2));
    assert_eq!(shown[0].breakpoint.as_deref(), Some("mobile"));
    assert_eq!(
        shown[1].reason.as_deref(),
        Some("plugins.error.not_available")
    );
    // The sentence's parameter goes with its key, so `{service}` never shows raw (RB-080-98).
    assert_eq!(
        shown[1].reason_params.get("service").map(String::as_str),
        Some("sidecar")
    );
    assert_eq!(shown[2].reason.as_deref(), Some(SKIP_CANCELLED_KEY));
    assert!(shown[2].reason_params.is_empty());
    assert_eq!(shown[2].breakpoint, None, "a view without a breakpoint");
}

#[test]
fn a_view_of_another_engine_is_skipped_with_the_engine_named() {
    let unserved = rb_plugin_types::Failure::engine_unserved("evaluate", 1, "webkit");
    let shown = ipc_jobs(
        &[job("a11y-axe", 1, JobState::ended_by(&unserved, Some(1)))],
        &BTreeMap::from([(1, "mobile".to_owned())]),
    );
    assert_eq!(shown[0].state, CheckJobState::Skipped);
    assert_eq!(shown[0].reason.as_deref(), Some(SKIP_ENGINE_KEY));
    assert_eq!(
        shown[0].reason_params.get("engine").map(String::as_str),
        Some("WebKit"),
        "the engine's name for people"
    );
}

#[test]
fn a_duration_reads_in_tenths_then_whole_seconds() {
    assert_eq!(seconds_text(Duration::from_millis(0)), "0.0");
    assert_eq!(seconds_text(Duration::from_millis(1240)), "1.3");
    assert_eq!(seconds_text(Duration::from_millis(9_950)), "10");
    assert_eq!(seconds_text(Duration::from_millis(42_100)), "43");
}

#[tokio::test(start_paused = true)]
async fn the_panel_hears_of_the_run_throttled_and_the_last_change_is_never_lost() {
    let (ui, core) = setup();
    let shared = Shared(sink(core));
    shared.update(&[job("seo", 1, JobState::Queued)], None);
    shared.update(&[job("seo", 1, JobState::Running)], None);
    shared.update(&[job("seo", 1, JobState::Running)], None);
    assert_eq!(events(&ui, "checksProgress").len(), 1, "the rest waits");
    tokio::time::sleep(THROTTLE * 2).await;
    let sent = events(&ui, "checksProgress");
    assert_eq!(sent.len(), 2, "one late send for both changes");
    assert_eq!(sent[1]["jobs"][0]["state"], "running");
    assert_eq!(sent[1]["estimateMs"], 4000);
    assert_eq!(sent[1]["finished"], false);
}

#[tokio::test(start_paused = true)]
async fn a_plugin_through_sends_its_findings_at_once_and_the_end_closes_the_run() {
    let (ui, core) = setup();
    let panel = sink(Arc::clone(&core));
    let shared = Shared(Arc::clone(&panel));
    shared.update(&[job("seo", 1, JobState::Queued)], None);
    shared.update(
        &[job("seo", 1, JobState::Done { findings: 0 })],
        Some(&empty_file()),
    );
    let findings = events(&ui, "findings");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["partial"], true);
    assert_eq!(events(&ui, "checksProgress").len(), 2, "not held back");

    shared.update(&[job("seo", 1, JobState::Running)], None);
    panel.finish(&RunOutcome {
        file: empty_file(),
        jobs: vec![job("seo", 1, JobState::Done { findings: 0 })],
        cancelled: false,
    });
    tokio::time::sleep(THROTTLE * 2).await;
    let sent = events(&ui, "checksProgress");
    let last = sent.last().expect("sent");
    assert_eq!(last["finished"], true, "a late send never undoes the end");
    let asked: ShellToCore =
        serde_json::from_value(json!({ "type": "getChecksProgress" })).expect("a shell message");
    let answer = super::super::ipc(&core, &asked)
        .expect("this module's")
        .expect("answered");
    assert!(matches!(answer, CoreToShell::ChecksProgress(progress) if progress.finished));
}

/// A late send that passed its check before the end must not undo the end (review c20, 1).
#[tokio::test]
async fn a_send_after_the_end_never_undoes_the_finished_progress() {
    let (ui, core) = setup();
    let panel = sink(Arc::clone(&core));
    panel.finish(&RunOutcome {
        file: empty_file(),
        jobs: vec![job("seo", 1, JobState::Done { findings: 0 })],
        cancelled: false,
    });
    let late = panel.progress(vec![], false, false);
    panel.send(late);
    assert_eq!(
        last_progress().map(|progress| progress.finished),
        Some(true)
    );
    let sent = events(&ui, "checksProgress");
    assert_eq!(
        sent.last().expect("sent")["finished"],
        true,
        "nothing after the end"
    );
}
