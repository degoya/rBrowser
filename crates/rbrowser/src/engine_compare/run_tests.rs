//! The acceptance of RB-094-03 over the core's run, without a browser: three engines whose
//! "rendering" is a fake backend each - Firefox wraps the one element onto a second line, the
//! others do not -, run at once through their own pools. Exactly that difference is reported
//! for the pairs with Firefox; `report.json` and `report.html` are written; a second run while
//! one runs is refused.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rb_config::EngineKind;
use rb_engine::chromium::batch::BatchOptions;
use rb_engine::chromium::hidden::{Loaded, RenderBackend, RenderId, RenderSpec, WaitSpec};
use rb_engine::{Emulation, EngineError};
use rb_visual::sitemap::{RunBreakpoint, RunState};
use rb_visual::{DiffOptions, Rgba, encode_png};

use super::*;
use crate::compare::report::PairStatus;
use crate::engine_compare::plan::SkippedEngine;
use crate::engine_compare::report::EngineRun;

/// One engine's page: 60 x 30 white, one black element; `wraps` puts a second line under it.
struct Engine {
    wraps: bool,
    next: AtomicU32,
    loaded: Mutex<Vec<String>>,
    /// Holds every screenshot until it has a permit (the busy test).
    gate: Option<Arc<tokio::sync::Semaphore>>,
}

impl Engine {
    fn new(wraps: bool) -> Arc<Self> {
        Arc::new(Self {
            wraps,
            next: AtomicU32::new(1),
            loaded: Mutex::new(Vec::new()),
            gate: None,
        })
    }
}

#[async_trait]
impl RenderBackend for Engine {
    async fn open(&self, _: &RenderSpec) -> Result<RenderId, EngineError> {
        Ok(RenderId(self.next.fetch_add(1, Ordering::Relaxed)))
    }

    async fn navigate(&self, _: RenderId, url: &str) -> Result<Loaded, EngineError> {
        lock(&self.loaded).push(url.to_owned());
        Ok(Loaded::default())
    }

    async fn wait(&self, _: RenderId, _: &WaitSpec) -> Result<(), EngineError> {
        Ok(())
    }

    async fn screenshot(&self, _: RenderId, _: bool) -> Result<Vec<u8>, EngineError> {
        if let Some(gate) = &self.gate {
            let _permit = gate.acquire().await.map_err(|_| EngineError::Closed)?;
        }
        let mut page = Rgba::new(60, 30, [255, 255, 255, 255])
            .map_err(|error| EngineError::Invalid(error.to_string()))?;
        page.fill(10, 5, 20, 10, [0, 0, 0, 255]);
        if self.wraps {
            page.fill(10, 18, 20, 8, [0, 0, 0, 255]);
        }
        encode_png(&page).map_err(|error| EngineError::Invalid(error.to_string()))
    }

    async fn close(&self, _: RenderId) {}
}

fn new_run(root: &std::path::Path, backends: Vec<Arc<dyn RenderBackend>>) -> NewEngineRun {
    let engines = [
        EngineKind::Chromium,
        EngineKind::Firefox,
        EngineKind::Webkit,
    ];
    let run = EngineRun::new(
        "https://example.test/fixture",
        RunBreakpoint {
            name: "desktop".to_owned(),
            label: "Desktop".to_owned(),
            width: 60,
            height: 30,
            dpr: 1.0,
        },
        &engines[..backends.len()],
        Vec::new(),
        0.0,
    );
    NewEngineRun {
        root: root.to_path_buf(),
        now_unix: 1_763_000_000,
        run,
        spec: RenderSpec {
            breakpoint: "desktop".to_owned(),
            emulation: Emulation {
                width: 60,
                height: 30,
                ..Emulation::default()
            },
            session: "default".to_owned(),
        },
        backends,
        batch: BatchOptions {
            parallel: 1,
            host_interval: Duration::ZERO,
            full_page: false,
            ..BatchOptions::default()
        },
        options: DiffOptions {
            threshold: 0.1,
            include_aa: false,
        },
        texts: Arc::new(|key: &str| format!("[{key}]")),
    }
}

#[tokio::test]
async fn the_element_that_wraps_only_in_firefox_is_the_one_difference_reported() {
    let root = tempfile::tempdir().expect("temp");
    let (chromium, firefox, webkit) = (Engine::new(false), Engine::new(true), Engine::new(false));
    let backends: Vec<Arc<dyn RenderBackend>> = vec![
        chromium.clone() as Arc<dyn RenderBackend>,
        firefox.clone(),
        webkit.clone(),
    ];
    let runs = EngineRuns::new();
    let summary = tokio::time::timeout(
        Duration::from_secs(20),
        runs.run(new_run(root.path(), backends)),
    )
    .await
    .expect("ends in time")
    .expect("runs");
    assert_eq!(summary.state, RunState::Finished);
    for engine in [&chromium, &firefox, &webkit] {
        assert_eq!(*lock(&engine.loaded), ["https://example.test/fixture"]);
    }
    let pairs: Vec<(&str, &str, PairStatus)> = summary
        .pairs
        .iter()
        .map(|pair| (pair.a.as_str(), pair.b.as_str(), pair.status))
        .collect();
    assert_eq!(
        pairs,
        [
            ("chromium", "firefox", PairStatus::Differs),
            ("chromium", "webkit", PairStatus::Same),
            ("firefox", "webkit", PairStatus::Differs),
        ]
    );
    assert_eq!(summary.differing, 2);
    for pair in summary
        .pairs
        .iter()
        .filter(|pair| pair.status == PairStatus::Differs)
    {
        let area = pair.regions.first().expect("the second line");
        assert_eq!((area.x, area.y, area.width, area.height), (10, 18, 20, 8));
        let image = pair.image.as_deref().expect("a diff image");
        assert!(std::path::Path::new(image).is_file(), "{image}");
    }
    for shot in &summary.shots {
        let file = shot.file.as_deref().expect("taken");
        assert!(std::path::Path::new(file).is_file(), "{file}");
    }
    let report = summary.report.clone().expect("report.html");
    let html = std::fs::read_to_string(&report).expect("reads");
    assert!(
        html.contains("[title]") && html.contains("chromium / firefox"),
        "{html}"
    );
    let json = std::fs::read_to_string(std::path::Path::new(&summary.folder).join("report.json"))
        .expect("report.json");
    let record: EngineRun = serde_json::from_str(&json).expect("reads back");
    assert_eq!(record.run.state, RunState::Finished);
    assert_eq!(record.url, "https://example.test/fixture");
    assert!(
        summary
            .folder
            .replace('\\', "/")
            .contains(".rbrowser/out/engine-compare/"),
        "{}",
        summary.folder
    );
}

#[tokio::test]
async fn one_engine_alone_is_taken_and_compared_with_nothing() {
    let root = tempfile::tempdir().expect("temp");
    let mut new = new_run(
        root.path(),
        vec![Engine::new(false) as Arc<dyn RenderBackend>],
    );
    new.run.skipped = vec![SkippedEngine {
        engine: "firefox".to_owned(),
        reason: "Firefox is not installed".to_owned(),
    }];
    let summary = EngineRuns::new().run(new).await.expect("runs");
    assert_eq!(summary.shots.len(), 1);
    assert!(summary.pairs.is_empty());
    assert_eq!(summary.skipped.len(), 1, "reported, not an error");
}

#[tokio::test]
async fn a_second_run_while_one_runs_is_refused_and_the_slot_frees_after() {
    let root = tempfile::tempdir().expect("temp");
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    let held = Arc::new(Engine {
        wraps: false,
        next: AtomicU32::new(1),
        loaded: Mutex::new(Vec::new()),
        gate: Some(gate.clone()),
    });
    let runs = EngineRuns::new();
    let first = {
        let (runs, new) = (
            runs.clone(),
            new_run(root.path(), vec![held as Arc<dyn RenderBackend>]),
        );
        tokio::spawn(async move { runs.run(new).await })
    };
    // Wait until the first run holds the slot.
    tokio::time::timeout(Duration::from_secs(20), async {
        while !runs.is_busy() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the first run starts");
    let refused = runs
        .run(new_run(
            root.path(),
            vec![Engine::new(false) as Arc<dyn RenderBackend>],
        ))
        .await
        .expect_err("busy");
    assert_eq!(refused.code(), "engine_compare.error.busy");
    gate.add_permits(1);
    first.await.expect("joins").expect("the first run ends");
    assert!(!runs.is_busy());
}

#[tokio::test]
async fn a_backend_missing_for_an_engine_fails_the_run_before_any_shot() {
    let root = tempfile::tempdir().expect("temp");
    let mut new = new_run(
        root.path(),
        vec![Engine::new(false) as Arc<dyn RenderBackend>],
    );
    new.backends.clear();
    let error = EngineRuns::new().run(new).await.expect_err("no backend");
    assert_eq!(error.code(), "engine_compare.error.failed");
}
