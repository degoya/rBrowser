use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rb_plugin_types::{CheckTarget, FindingsFile, Severity, codes};
use serde_json::json;

use super::{
    Cancel, JobProgress, JobState, PluginTiming, ProgressSink, RunControl, SkipReason, estimate_ms,
    timings_of,
};
use crate::checks::testing::{FakeSource, finding, rule};
use crate::checks::{CheckRunner, CheckSource, ChecksSettings, RunRequest, RunView};

/// Every update a run told, in order.
#[derive(Default)]
struct Recorder {
    updates: Mutex<Vec<(Vec<JobProgress>, Option<FindingsFile>)>>,
}

impl ProgressSink for Recorder {
    fn update(&self, jobs: &[JobProgress], partial: Option<&FindingsFile>) {
        self.updates
            .lock()
            .expect("updates")
            .push((jobs.to_vec(), partial.cloned()));
    }
}

impl Recorder {
    fn updates(&self) -> Vec<(Vec<JobProgress>, Option<FindingsFile>)> {
        self.updates.lock().expect("updates").clone()
    }
}

fn request(views: u32) -> RunRequest {
    RunRequest {
        run_id: "run-0000000000001".to_owned(),
        started_at: 1,
        target: CheckTarget::All,
        views: (1..=views)
            .map(|id| RunView {
                id,
                breakpoint: Some(format!("bp{id}")),
                url: Some("https://site.test/".to_owned()),
            })
            .collect(),
        url: None,
        plugins: None,
        args: "{}".to_owned(),
    }
}

fn source(id: &str, delay_ms: u64) -> FakeSource {
    let mut source = FakeSource::new(
        id,
        vec![rule("images", None)],
        vec![finding("images", Severity::Error, "img")],
    );
    source.delay = Duration::from_millis(delay_ms);
    source
}

fn sources(list: Vec<FakeSource>) -> Vec<Arc<dyn CheckSource>> {
    list.into_iter()
        .map(|source| Arc::new(source) as Arc<dyn CheckSource>)
        .collect()
}

fn runner(parallel: u32) -> CheckRunner {
    CheckRunner::new(&ChecksSettings::from_value(Some(
        &json!({ "parallel": parallel }),
    )))
}

fn control(recorder: &Arc<Recorder>) -> RunControl {
    RunControl {
        cancel: Arc::new(Cancel::default()),
        sink: Some(Arc::clone(recorder) as Arc<dyn ProgressSink>),
    }
}

fn states(jobs: &[JobProgress]) -> Vec<String> {
    jobs.iter()
        .map(|job| {
            let state = match &job.state {
                JobState::Queued => "queued".to_owned(),
                JobState::Running => "running".to_owned(),
                JobState::Done { findings } => format!("done {findings}"),
                JobState::Failed { code, .. } => {
                    format!("failed {}", code.as_deref().unwrap_or("-"))
                }
                JobState::Skipped { reason, .. } => format!("skipped {reason:?}"),
            };
            format!("{}@{}: {state}", job.plugin, job.view.unwrap_or(0))
        })
        .collect()
}

#[tokio::test(start_paused = true)]
async fn the_run_tells_each_plugin_and_view_as_it_goes() {
    let recorder = Arc::new(Recorder::default());
    let outcome = runner(1)
        .run_with(
            &sources(vec![source("fast", 10), source("slow", 50)]),
            request(2),
            &control(&recorder),
        )
        .await;
    let updates = recorder.updates();
    assert_eq!(
        states(&updates[0].0),
        [
            "fast@1: queued",
            "fast@2: queued",
            "slow@1: queued",
            "slow@2: queued"
        ],
        "every job is planned before one runs"
    );
    assert_eq!(
        states(&updates[1].0)[0],
        "fast@1: running",
        "a job is told as it starts"
    );
    assert!(
        updates
            .iter()
            .any(|(jobs, _)| states(jobs).contains(&"fast@1: done 1".to_owned())),
        "a job is told as it ends, with its count"
    );
    assert_eq!(
        states(&outcome.jobs),
        [
            "fast@1: done 1",
            "fast@2: done 1",
            "slow@1: done 1",
            "slow@2: done 1"
        ]
    );
    assert!(!outcome.cancelled);
    assert!(outcome.jobs.iter().all(|job| job.elapsed_ms.is_some()));
}

#[tokio::test(start_paused = true)]
async fn the_findings_of_a_plugin_come_as_soon_as_it_is_through() {
    let recorder = Arc::new(Recorder::default());
    runner(1)
        .run_with(
            &sources(vec![source("fast", 10), source("slow", 50)]),
            request(2),
            &control(&recorder),
        )
        .await;
    let partials: Vec<FindingsFile> = recorder
        .updates()
        .into_iter()
        .filter_map(|(_, partial)| partial)
        .collect();
    assert_eq!(partials.len(), 2, "one per plugin through");
    assert_eq!(partials[0].summary.total, 1, "fast's merged finding");
    assert_eq!(partials[0].findings[0].plugins, ["fast"]);
    assert_eq!(partials[0].findings[0].views, [1, 2]);
    assert_eq!(partials[1].summary.total, 2, "then slow's beside it");
}

#[tokio::test(start_paused = true)]
async fn a_cancelled_run_keeps_what_the_finished_jobs_found() {
    let recorder = Arc::new(Recorder::default());
    let control = control(&recorder);
    let stopper = Arc::clone(&control.cancel);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(30)).await;
        stopper.cancel();
    });
    let outcome = runner(1)
        .run_with(
            &sources(vec![source("fast", 10), source("slow", 1000)]),
            request(1),
            &control,
        )
        .await;
    assert!(outcome.cancelled);
    assert_eq!(
        states(&outcome.jobs),
        ["fast@1: done 1", "slow@1: skipped Cancelled"]
    );
    assert_eq!(outcome.file.summary.total, 1, "fast's finding stays");
    assert!(outcome.file.failures.is_empty(), "a cancel is no failure");
}

#[tokio::test(start_paused = true)]
async fn a_failure_is_told_with_its_code_and_a_plugin_without_rules_is_skipped() {
    let mut broken = source("broken", 10);
    broken.fail = true;
    let silent = FakeSource::new("silent", Vec::new(), Vec::new());
    let recorder = Arc::new(Recorder::default());
    let outcome = runner(2)
        .run_with(
            &sources(vec![broken, silent]),
            request(1),
            &control(&recorder),
        )
        .await;
    assert_eq!(
        states(&outcome.jobs),
        [
            "silent@0: skipped RulesOff",
            format!("broken@1: failed {}", codes::TIMED_OUT).as_str()
        ]
    );
    assert_eq!(outcome.file.failures.len(), 1);
}

#[tokio::test(start_paused = true)]
async fn run_without_a_sink_is_the_plain_run() {
    let file = runner(2)
        .run(&sources(vec![source("fast", 1)]), request(2))
        .await;
    assert_eq!(file.summary.total, 1);
}

#[test]
fn the_wire_form_of_a_job_is_flat() {
    let job = JobProgress {
        plugin: "seo".to_owned(),
        view: Some(2),
        state: JobState::Done { findings: 3 },
        elapsed_ms: Some(1200),
    };
    assert_eq!(
        serde_json::to_value(&job).expect("json"),
        json!({ "plugin": "seo", "view": 2, "state": "done", "findings": 3, "elapsedMs": 1200 })
    );
    let skipped = JobProgress {
        plugin: "seo".to_owned(),
        view: None,
        state: JobState::skipped(SkipReason::RulesOff),
        elapsed_ms: None,
    };
    assert_eq!(
        serde_json::to_value(&skipped).expect("json"),
        json!({ "plugin": "seo", "state": "skipped", "reason": "rulesOff" })
    );
}

fn job(plugin: &str, state: JobState, ms: Option<u64>) -> JobProgress {
    JobProgress {
        plugin: plugin.to_owned(),
        view: Some(1),
        state,
        elapsed_ms: ms,
    }
}

#[test]
fn timings_count_only_plugins_whose_every_job_answered() {
    let jobs = [
        job("seo", JobState::Done { findings: 1 }, Some(400)),
        job(
            "seo",
            JobState::Failed {
                code: None,
                params: rb_plugin_types::MessageParams::new(),
            },
            Some(600),
        ),
        job("axe", JobState::skipped(SkipReason::Cancelled), None),
        job("axe", JobState::Done { findings: 0 }, Some(100)),
    ];
    let timings = timings_of(&jobs);
    assert_eq!(
        timings,
        BTreeMap::from([(
            "seo".to_owned(),
            PluginTiming {
                work_ms: 1000,
                longest_ms: 600
            }
        )])
    );
}

#[test]
fn the_estimate_spreads_the_work_over_the_slots_but_not_below_the_longest_job() {
    let last = BTreeMap::from([
        (
            "seo".to_owned(),
            PluginTiming {
                work_ms: 4000,
                longest_ms: 1000,
            },
        ),
        (
            "lighthouse".to_owned(),
            PluginTiming {
                work_ms: 30_000,
                longest_ms: 30_000,
            },
        ),
    ]);
    let both = ["seo".to_owned(), "lighthouse".to_owned()];
    assert_eq!(
        estimate_ms(&last, &both, 4),
        Some(30_000),
        "the longest job"
    );
    assert_eq!(estimate_ms(&last, &["seo".to_owned()], 4), Some(1000));
    assert_eq!(estimate_ms(&last, &["seo".to_owned()], 2), Some(2000));
    assert_eq!(
        estimate_ms(&last, &["seo".to_owned(), "new".to_owned()], 4),
        None,
        "no estimate while a plugin has no timing"
    );
    assert_eq!(estimate_ms(&last, &[], 4), None);
}

#[tokio::test(start_paused = true)]
async fn a_describe_that_panics_fails_its_plugin_and_no_other_plugin_takes_its_place() {
    let mut broken = source("broken", 10);
    broken.panic_describe = true;
    let recorder = Arc::new(Recorder::default());
    let outcome = runner(2)
        .run_with(
            &sources(vec![broken, source("fast", 10)]),
            request(1),
            &control(&recorder),
        )
        .await;
    assert_eq!(
        states(&outcome.jobs),
        [
            format!("broken@0: failed {}", codes::HOST_FAILED).as_str(),
            "fast@1: done 1"
        ]
    );
    assert_eq!(outcome.file.findings[0].plugins, ["fast"]);
    assert_eq!(outcome.file.failures.len(), 1);
    assert_eq!(outcome.file.failures[0].plugin, "broken");
}

#[tokio::test(start_paused = true)]
async fn a_run_that_panics_is_a_failure_of_its_plugin_not_a_cancel() {
    let mut broken = source("broken", 10);
    broken.panic_run = true;
    let recorder = Arc::new(Recorder::default());
    let outcome = runner(2)
        .run_with(&sources(vec![broken]), request(1), &control(&recorder))
        .await;
    assert_eq!(
        states(&outcome.jobs),
        [format!("broken@1: failed {}", codes::HOST_FAILED).as_str()]
    );
    assert!(!outcome.cancelled);
    assert_eq!(outcome.file.failures.len(), 1);
    assert_eq!(
        outcome.file.failures[0].code.as_deref(),
        Some(codes::HOST_FAILED)
    );
}

#[test]
fn a_plugin_that_ran_nothing_takes_no_time_and_keeps_the_estimate() {
    let off = JobProgress {
        plugin: "off".to_owned(),
        view: None,
        state: JobState::skipped(SkipReason::RulesOff),
        elapsed_ms: None,
    };
    let undescribed = JobProgress {
        plugin: "broken".to_owned(),
        view: None,
        state: JobState::Failed {
            code: None,
            params: rb_plugin_types::MessageParams::new(),
        },
        elapsed_ms: None,
    };
    let timings = timings_of(&[
        off,
        undescribed,
        job("seo", JobState::Done { findings: 1 }, Some(500)),
    ]);
    assert_eq!(timings.get("off"), Some(&PluginTiming::default()));
    assert_eq!(timings.get("broken"), Some(&PluginTiming::default()));
    let plugins = ["seo".to_owned(), "off".to_owned(), "broken".to_owned()];
    assert_eq!(estimate_ms(&timings, &plugins, 4), Some(500));
}
