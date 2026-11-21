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
        None,
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

/// A plugin's own run after a full one (RB-094-14): its rows follow, the panel hears of them.
#[tokio::test]
async fn a_plugins_own_run_updates_its_rows_of_the_finished_progress() {
    let (ui, core) = setup();
    let panel = sink(Arc::clone(&core));
    panel.finish(&RunOutcome {
        file: empty_file(),
        jobs: vec![
            job("seo", 1, JobState::Done { findings: 3 }),
            job("a11y", 1, JobState::Done { findings: 1 }),
        ],
        cancelled: false,
    });
    let mut file = empty_file();
    file.views = vec![rb_plugin_types::CheckedView {
        id: 1,
        breakpoint: Some("mobile".to_owned()),
    }];
    follow_reported(&core, &file, "seo", true);
    let shown = last_progress().expect("kept");
    assert_eq!(shown.run_id, "run-0000000000001", "still the full run");
    assert_eq!(shown.jobs.len(), 2);
    assert_eq!(shown.jobs[0].plugin, "seo");
    assert_eq!(shown.jobs[0].findings, Some(0), "the new count");
    assert_eq!(shown.jobs[1].findings, Some(1), "the other plugin's kept");
    let sent = events(&ui, "checksProgress");
    assert_eq!(sent.last().expect("sent")["jobs"][0]["findings"], 0);

    follow_reported(&core, &file, "seo", false);
    let replaced = last_progress().expect("kept");
    assert_eq!(replaced.jobs.len(), 1, "a replaced run's rows alone");
    assert!(replaced.finished);
}

#[tokio::test]
async fn a_plugins_own_run_leaves_the_progress_of_a_run_going_alone() {
    let (ui, core) = setup();
    let shared = Shared(sink(Arc::clone(&core)));
    shared.update(&[job("seo", 1, JobState::Running)], None);
    let before = events(&ui, "checksProgress").len();
    follow_reported(&core, &empty_file(), "seo", true);
    assert_eq!(events(&ui, "checksProgress").len(), before);
    assert_eq!(
        last_progress().map(|progress| progress.finished),
        Some(false)
    );
}

/// A run of chosen checks (RB-094-17): the panel keeps the other checks' findings while it goes
/// and their rows at its end.
#[tokio::test]
async fn a_run_of_chosen_checks_keeps_the_other_checks_findings_and_rows() {
    let (ui, core) = setup();
    let mut last = empty_file();
    last.plugins = vec!["seo".to_owned(), "a11y".to_owned()];
    last.findings = vec![
        serde_json::from_value(json!({
            "id": "alt", "ruleId": "a11y.alt", "plugins": ["a11y"], "severity": "error",
            "message": { "key": "plugin.a11y.findings.alt" }, "views": [], "breakpoints": []
        }))
        .expect("a finding"),
    ];
    let request = rb_plugin_host::RunRequest {
        run_id: "run-0000000000002".to_owned(),
        started_at: 2,
        target: rb_plugin_types::CheckTarget::All,
        views: Vec::new(),
        url: None,
        plugins: Some(vec!["seo".to_owned()]),
        args: "{}".to_owned(),
    };
    let before = ChecksProgress {
        run_id: "run-0000000000001".to_owned(),
        elapsed_ms: 100,
        estimate_ms: None,
        jobs: ipc_jobs(
            &[
                job("seo", 1, JobState::Done { findings: 3 }),
                job("a11y", 1, JobState::Done { findings: 1 }),
            ],
            &BTreeMap::new(),
        ),
        finished: true,
        cancelled: false,
    };
    let base = super::super::selection::Base::of(
        Some((last, BTreeMap::new())),
        Some(&before),
        &request,
        &["seo".to_owned()],
    )
    .expect("the same views");
    let panel = PanelSink::start(
        Arc::clone(&core),
        request.run_id.clone(),
        BTreeMap::new(),
        None,
        Default::default(),
        Arc::new(Cancel::default()),
        Some(Arc::new(base)),
    );
    Shared(Arc::clone(&panel)).update(
        &[job("seo", 1, JobState::Done { findings: 0 })],
        Some(&empty_file()),
    );
    let findings = events(&ui, "findings");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["partial"], true);
    assert_eq!(
        findings[0]["run"]["findings"][0]["id"], "alt",
        "a11y's stay"
    );
    panel.finish(&RunOutcome {
        file: empty_file(),
        jobs: vec![job("seo", 1, JobState::Done { findings: 0 })],
        cancelled: false,
    });
    let shown = last_progress().expect("kept");
    let rows: Vec<(&str, Option<u32>)> = shown
        .jobs
        .iter()
        .map(|job| (job.plugin.as_str(), job.findings))
        .collect();
    assert_eq!(rows, vec![("seo", Some(0)), ("a11y", Some(1))]);
    assert_eq!(shown.run_id, "run-0000000000002", "the new run's progress");
    assert!(shown.finished);
}
