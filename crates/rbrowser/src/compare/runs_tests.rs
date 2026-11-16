//! The acceptance of RB-080-03 over the core's run, without a browser: `local` and `staging`
//! identical but for one rule in `@media (min-width: 1024px)` - render views that paint a page
//! of `staging` differently from 1024 px on - over four breakpoints: the run reports a
//! difference exactly in the breakpoints of 1024 px and more, 0 % in the others, and two
//! identical environments differ nowhere. `report.json` and `report.html` are written.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rb_engine::chromium::batch::BatchOptions;
use rb_engine::chromium::hidden::{Loaded, RenderBackend, RenderId, RenderSpec, WaitSpec};
use rb_engine::{Emulation, EngineError};
use rb_visual::sitemap::RunBreakpoint;
use rb_visual::{Rgba, encode_png};
use tokio::sync::mpsc;

use super::*;
use crate::compare::report::{PairStatus, RunEnvironment, RunPath};

#[derive(Default)]
struct Pages {
    next: AtomicU32,
    views: Mutex<BTreeMap<RenderId, (u32, String)>>,
}

#[async_trait]
impl RenderBackend for Pages {
    async fn open(&self, spec: &RenderSpec) -> Result<RenderId, EngineError> {
        let id = RenderId(self.next.fetch_add(1, Ordering::Relaxed));
        lock(&self.views).insert(id, (spec.emulation.width, String::new()));
        Ok(id)
    }

    async fn navigate(&self, view: RenderId, url: &str) -> Result<Loaded, EngineError> {
        let mut views = lock(&self.views);
        let entry = views.get_mut(&view).ok_or(EngineError::Closed)?;
        entry.1 = url.to_owned();
        Ok(Loaded {
            status: Some(200),
            final_url: None,
        })
    }

    async fn wait(&self, _: RenderId, _: &WaitSpec) -> Result<(), EngineError> {
        Ok(())
    }

    async fn screenshot(&self, view: RenderId, _: bool) -> Result<Vec<u8>, EngineError> {
        let (width, url) = lock(&self.views)
            .get(&view)
            .cloned()
            .ok_or(EngineError::Closed)?;
        let side = width / 16;
        let mut page = Rgba::new(side, 20, [255, 255, 255, 255])
            .map_err(|e| EngineError::Invalid(e.to_string()))?;
        // The staging stylesheet: `@media (min-width: 1024px) { header { color: red } }`.
        if url.contains("staging") && width >= 1024 {
            page.fill(0, 0, side, 4, [255, 0, 0, 255]);
        }
        encode_png(&page).map_err(|e| EngineError::Invalid(e.to_string()))
    }

    async fn close(&self, view: RenderId) {
        lock(&self.views).remove(&view);
    }
}

struct Events(mpsc::UnboundedSender<Summary>);

impl RunEvents for Events {
    fn progress(&self, _: &str, _: usize, _: usize) {}

    fn ended(&self, summary: &Summary) {
        let _ = self.0.send(summary.clone());
    }
}

const WIDTHS: [(&str, u32); 4] = [
    ("mobile", 375),
    ("tablet", 768),
    ("laptop", 1024),
    ("wide", 1440),
];

fn new_run(root: &Path, staging: &str) -> NewRun {
    let environment = |id: &str, url: &str| RunEnvironment {
        id: id.to_owned(),
        url: url.to_owned(),
        color: None,
    };
    let run = EnvRun {
        environments: vec![
            environment("local", "http://localhost:3000"),
            environment("staging", staging),
        ],
        breakpoints: WIDTHS
            .iter()
            .map(|(name, width)| RunBreakpoint {
                name: (*name).to_owned(),
                label: (*name).to_owned(),
                width: *width,
                height: 800,
                dpr: 1.0,
            })
            .collect(),
        paths: vec![RunPath {
            path: "/about".to_owned(),
            folder: "01-about".to_owned(),
        }],
        ..EnvRun::default()
    };
    let specs = WIDTHS
        .iter()
        .map(|(name, width)| RenderSpec {
            breakpoint: (*name).to_owned(),
            emulation: Emulation {
                width: *width,
                height: 800,
                ..Emulation::default()
            },
            session: "default".to_owned(),
        })
        .collect();
    NewRun {
        root: root.to_path_buf(),
        now_unix: 1_763_000_000,
        run,
        specs,
        batch: BatchOptions {
            parallel: 2,
            host_interval: Duration::ZERO,
            ..BatchOptions::default()
        },
        options: DiffOptions {
            threshold: 0.1,
            include_aa: false,
        },
        texts: Arc::new(|key: &str| key.to_owned()),
    }
}

async fn finished(runs: &Arc<Runs>, new: NewRun) -> (Started, Summary) {
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let services = Services {
        backend: Arc::new(Pages::default()),
        events: Arc::new(Events(sender)),
    };
    let started = runs.start(new, services).await.expect("starts");
    let summary = tokio::time::timeout(Duration::from_secs(20), receiver.recv())
        .await
        .expect("ends in time")
        .expect("an end");
    (started, summary)
}

#[tokio::test]
async fn a_rule_from_1024_px_on_differs_exactly_in_the_wide_breakpoints() {
    let root = tempfile::tempdir().expect("temp");
    let runs = Runs::new();
    let (started, summary) =
        finished(&runs, new_run(root.path(), "http://staging.localhost:3001")).await;
    assert_eq!((started.shots, started.diffs), (8, 4));
    assert_eq!(summary.state, RunState::Finished);
    assert_eq!(summary.shots_done, 8);
    let by_breakpoint: Vec<(&str, PairStatus, f64)> = summary
        .pairs
        .iter()
        .map(|pair| (pair.breakpoint.as_str(), pair.status, pair.percent))
        .collect();
    assert_eq!(
        by_breakpoint,
        [
            ("mobile", PairStatus::Same, 0.0),
            ("tablet", PairStatus::Same, 0.0),
            ("laptop", PairStatus::Differs, 20.0),
            ("wide", PairStatus::Differs, 20.0),
        ]
    );
    assert_eq!(summary.differing, 2);
    let report = summary.report.clone().expect("report.html");
    let html = std::fs::read_to_string(&report).expect("reads");
    assert!(html.contains("01-about/diff-1-2--3-laptop.png"), "{html}");
    let folder = Path::new(&started.folder);
    assert!(folder.join("01-about/1-local--1-mobile.png").is_file());
    assert!(folder.join("01-about/diff-1-2--4-wide.png").is_file());
    assert!(
        !folder.join("01-about/diff-1-2--1-mobile.png").exists(),
        "no image without a difference"
    );
    let again = runs
        .status(root.path(), Some(&started.run_id))
        .expect("from report.json");
    assert_eq!(again.pairs, summary.pairs);
    assert_eq!(again.report.as_deref(), Some(report.as_str()));
    assert_eq!(
        runs.status(root.path(), None).expect("latest").run_id,
        started.run_id
    );
}

#[tokio::test]
async fn two_identical_environments_differ_in_no_breakpoint() {
    let root = tempfile::tempdir().expect("temp");
    let (_, summary) = finished(&Runs::new(), new_run(root.path(), "http://127.0.0.1:3000")).await;
    assert_eq!(summary.differing, 0);
    assert!(
        summary
            .pairs
            .iter()
            .all(|pair| pair.status == PairStatus::Same && pair.percent == 0.0)
    );
}

#[tokio::test]
async fn one_run_at_a_time_and_an_unknown_run_is_not_found() {
    let root = tempfile::tempdir().expect("temp");
    let runs = Runs::new();
    let (sender, _receiver) = mpsc::unbounded_channel();
    let services = Services {
        backend: Arc::new(Pages::default()),
        events: Arc::new(Events(sender)),
    };
    let first = runs.start(
        new_run(root.path(), "http://staging.localhost"),
        services.clone(),
    );
    let second = runs.start(new_run(root.path(), "http://staging.localhost"), services);
    let (first, second) = tokio::join!(first, second);
    assert!(first.is_ok() != second.is_ok(), "exactly one starts");
    let refused = first.err().or(second.err()).expect("one refused");
    assert_eq!(refused.code(), "env_compare.error.busy");
    assert_eq!(
        runs.status(root.path(), Some("nothing"))
            .map_err(|e| e.code())
            .err(),
        Some("env_compare.error.not_found")
    );
}

/// Render views whose pages never load: a run that only a cancel ends.
struct Stuck;

#[async_trait]
impl RenderBackend for Stuck {
    async fn open(&self, _: &RenderSpec) -> Result<RenderId, EngineError> {
        Ok(RenderId(1))
    }

    async fn navigate(&self, _: RenderId, _: &str) -> Result<Loaded, EngineError> {
        std::future::pending().await
    }

    async fn wait(&self, _: RenderId, _: &WaitSpec) -> Result<(), EngineError> {
        Ok(())
    }

    async fn screenshot(&self, _: RenderId, _: bool) -> Result<Vec<u8>, EngineError> {
        Err(EngineError::Closed)
    }

    async fn close(&self, _: RenderId) {}
}

/// Charge 9 review M2: a run can be cancelled; it ends as cancelled with its reports written and
/// the slot free for the next run.
#[tokio::test]
async fn a_running_comparison_is_cancelled() {
    let root = tempfile::tempdir().expect("temp");
    let runs = Runs::new();
    assert_eq!(
        runs.cancel(root.path(), None).map_err(|e| e.code()).err(),
        Some("env_compare.error.not_found"),
        "nothing to cancel"
    );
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let services = Services {
        backend: Arc::new(Stuck),
        events: Arc::new(Events(sender)),
    };
    let started = runs
        .start(new_run(root.path(), "http://staging.localhost"), services)
        .await
        .expect("starts");
    let (id, running) = runs.cancel(root.path(), None).expect("cancelled");
    assert_eq!((id.as_str(), running), (started.run_id.as_str(), true));
    let summary = tokio::time::timeout(Duration::from_secs(20), receiver.recv())
        .await
        .expect("ends in time")
        .expect("an end");
    assert_eq!(summary.state, RunState::Cancelled);
    assert!(summary.report.is_some(), "report.html is written");
    let (id, running) = runs
        .cancel(root.path(), Some(&started.run_id))
        .expect("known");
    assert_eq!((id.as_str(), running), (started.run_id.as_str(), false));
    assert_eq!(
        runs.cancel(root.path(), Some("nothing"))
            .map_err(|e| e.code())
            .err(),
        Some("env_compare.error.not_found")
    );
    let (again, _) = finished(&runs, new_run(root.path(), "http://127.0.0.1:3000")).await;
    assert_ne!(again.run_id, started.run_id, "the slot is free");
}
