//! One engine comparison, without the window: [`EngineRuns::run`] writes the run's folder and
//! `report.json`, takes every engine's shot at once - each engine through a pool and a batch of
//! its own (Chromium in a hidden render view, Firefox and WebKit in a window of their own, see
//! [`super::backend`]) -, diffs each engine pair as soon as both shots are in (the environment
//! comparison's sink, on a blocking thread), writes `report.json` again and `report.html` and
//! answers the summary. One run at a time; the backends are traits, so the whole run is tested
//! against fakes.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use rb_engine::chromium::batch::{BatchOptions, BatchTask, Cancel, run_batch};
use rb_engine::chromium::hidden::{RenderBackend, RenderPool, RenderSpec};
use rb_visual::DiffOptions;
use rb_visual::sitemap::RunState;
use rb_visual::sitemap_store::RunStore;

use super::plan::PlanError;
use super::report::{EngineRun, EngineSummary, OUT_DIR, render_html};
use crate::cdp::lock;
use crate::compare::report::{REPORT_HTML, REPORT_JSON, Summary};
use crate::compare::runs::{RunEvents, blocking, fresh_store_in};
use crate::compare::sink::Sink;

/// Why a comparison cannot run. The message starts with its stable code.
#[derive(Debug, thiserror::Error)]
pub enum EngineRunError {
    #[error("engine_compare.error.no_project: no project is open")]
    NoProject,
    #[error("engine_compare.error.busy: an engine comparison is running")]
    Busy,
    #[error(transparent)]
    Plan(#[from] PlanError),
    #[error("engine_compare.error.failed: {0}")]
    Failed(String),
}

impl EngineRunError {
    /// The stable code of the error.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoProject => "engine_compare.error.no_project",
            Self::Busy => "engine_compare.error.busy",
            Self::Plan(error) => error.code(),
            Self::Failed(_) => "engine_compare.error.failed",
        }
    }
}

impl From<crate::compare::runs::RunError> for EngineRunError {
    fn from(error: crate::compare::runs::RunError) -> Self {
        Self::Failed(error.to_string())
    }
}

impl From<rb_visual::VisualError> for EngineRunError {
    fn from(error: rb_visual::VisualError) -> Self {
        Self::Failed(error.to_string())
    }
}

/// The texts of `report.html`, by key.
pub type Texts = Arc<dyn Fn(&str) -> String + Send + Sync>;

/// A new run: the record with its engines, the breakpoint's spec, one backend per engine (in the
/// record's order), the shots and the diff.
pub struct NewEngineRun {
    pub root: PathBuf,
    pub now_unix: u64,
    pub run: EngineRun,
    pub spec: RenderSpec,
    pub backends: Vec<Arc<dyn RenderBackend>>,
    pub batch: BatchOptions,
    pub options: DiffOptions,
    pub texts: Texts,
}

/// The sink's follower: the shots in the log only; the caller hears the end from [`EngineRuns::run`].
struct Quiet;

impl RunEvents for Quiet {
    fn progress(&self, run_id: &str, done: usize, total: usize) {
        tracing::debug!(run_id, done, total, "an engine comparison took a shot");
    }

    fn ended(&self, _: &Summary) {}
}

/// The engine comparisons of one window: at most one at a time.
#[derive(Default)]
pub struct EngineRuns {
    busy: AtomicBool,
}

/// Holds the one-run slot; frees it when dropped.
struct Claim<'a>(&'a EngineRuns);

impl Drop for Claim<'_> {
    fn drop(&mut self) {
        self.0.busy.store(false, Ordering::SeqCst);
    }
}

fn save(store: &RunStore, run: &EngineRun) -> Result<(), EngineRunError> {
    let mut text = serde_json::to_string_pretty(run)
        .map_err(|error| EngineRunError::Failed(error.to_string()))?;
    text.push('\n');
    store.write_file(REPORT_JSON, text.as_bytes())?;
    Ok(())
}

impl EngineRuns {
    /// No run yet.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Whether a run is in progress.
    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }

    /// Runs `new` to its end and answers its summary.
    ///
    /// # Errors
    ///
    /// Another run is in progress, a backend is missing, or the folder cannot be written. A
    /// shot that fails is no error: the summary names it.
    pub async fn run(&self, new: NewEngineRun) -> Result<EngineSummary, EngineRunError> {
        if self.busy.swap(true, Ordering::SeqCst) {
            return Err(EngineRunError::Busy);
        }
        let _claim = Claim(self);
        let NewEngineRun {
            root,
            now_unix,
            mut run,
            spec,
            backends,
            batch,
            options,
            texts,
        } = new;
        if backends.len() != run.run.environments.len() {
            return Err(EngineRunError::Failed(format!(
                "{} backends for {} engines",
                backends.len(),
                run.run.environments.len()
            )));
        }
        let (run_id, store) = blocking(move || fresh_store_in(&root, &OUT_DIR, now_unix)).await??;
        run.run.run_id = run_id;
        run.run.created_unix = now_unix;
        run.run.state = RunState::Running;
        run.run.threshold = options.threshold();
        run.run.full_page = batch.full_page;
        let (saving, saved) = (store.clone(), run.clone());
        blocking(move || save(&saving, &saved)).await??;
        let shared = Arc::new(Mutex::new(run.run.clone()));
        let sink = Arc::new(Sink {
            run: shared.clone(),
            store: store.clone(),
            groups: Mutex::new(BTreeMap::new()),
            pairs: crate::compare::plan::pairs(backends.len()),
            options,
            max_ratio: run.run.max_ratio,
            events: Arc::new(Quiet),
        });
        let cancel = Arc::new(Cancel::default());
        let mut lanes = tokio::task::JoinSet::new();
        for (engine, backend) in backends.into_iter().enumerate() {
            let tasks: Vec<BatchTask> = run
                .run
                .shots
                .iter()
                .enumerate()
                .filter(|(_, shot)| shot.environment == engine)
                .map(|(key, shot)| BatchTask {
                    key,
                    url: shot.url.clone(),
                    spec: spec.clone(),
                })
                .collect();
            let (sink, cancel, batch) = (sink.clone(), cancel.clone(), batch.clone());
            lanes.spawn(async move {
                let pool = RenderPool::new(backend, 1);
                run_batch(pool.clone(), tasks, batch, sink, cancel).await;
                pool.shutdown().await;
            });
        }
        while lanes.join_next().await.is_some() {}
        run.run = {
            let mut done = lock(&shared);
            done.state = RunState::Finished;
            done.finished_unix = Some(crate::sitemap::runs::now_unix());
            done.clone()
        };
        let (writing, record) = (store.clone(), run.clone());
        let written = blocking(move || {
            save(&writing, &record)?;
            let html = render_html(&record, &*texts);
            Ok::<_, EngineRunError>(writing.write_file(REPORT_HTML, html.as_bytes())?)
        })
        .await;
        let report = match written {
            Ok(Ok(path)) => Some(path.display().to_string()),
            Ok(Err(error)) => {
                tracing::warn!(%error, "the engine comparison's report was not written");
                None
            }
            Err(error) => {
                tracing::warn!(%error, "the engine comparison's report was not written");
                None
            }
        };
        Ok(run.summary(&store.dir().display().to_string(), report))
    }
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
