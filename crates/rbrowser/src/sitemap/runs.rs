//! The runs of sitemap capture, without the window: [`Runs`] starts one (the plugin's
//! `discover` gives the pages, `run.json` is written, the batch goes on in the background),
//! resumes one from its `run.json` (what is done stays, a failed shot is tried again up to
//! [`MAX_ATTEMPTS`] attempts over all runs), cancels it, and answers its status. One run at a
//! time: the render views are the costly part, and two runs would share the host's throttle.
//!
//! The plugin, the render views and whoever follows the run are traits ([`PluginTools`],
//! `RenderBackend`, [`RunEvents`]), so the whole run - cancel at half and resume included - is
//! tested against fakes; `super::app` gives it the bundled plugin, the CDP render views and the
//! shell. When the batch ends (done or cancelled) `run.json` is written and the plugin's
//! gallery goes beside it as `index.html`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rb_engine::chromium::batch::{BatchOptions, BatchTask, Cancel};
use rb_engine::chromium::hidden::{RenderBackend, RenderSpec};
use rb_visual::sitemap::{Progress, RunState, SitemapRun};
use rb_visual::sitemap_store::RunStore;
use serde_json::Value;

use super::job::{Job, blocking, fresh_id, loadable, prepare, refused_scheme, spec_for};
use super::plan::{Breakpoints, PlanError};
use super::settings::MAX_ATTEMPTS;
use super::sink::CompareWith;
use super::tool::CaptureStarted;
use crate::cdp::lock;

pub use super::job::{now_unix, pages_of};
pub use super::status::{RunError, status_of};

/// The plugin's tools a run calls (blocking; a run calls them on a blocking thread).
pub trait PluginTools: Send + Sync {
    /// `discover` with `args`: its answer.
    fn discover(&self, args: &Value) -> Result<Value, String>;
    /// `gallery` of `run` (`run.json`): the HTML.
    fn gallery(&self, run: &Value) -> Result<String, String>;
}

/// How a run ended.
#[derive(Clone, Debug, PartialEq)]
pub struct Ended {
    pub run_id: String,
    pub state: RunState,
    pub progress: Progress,
    pub folder: PathBuf,
    /// The gallery, when it was written.
    pub gallery: Option<PathBuf>,
}

/// Whoever follows the runs.
pub trait RunEvents: Send + Sync {
    /// Run `run_id` took a shot.
    fn progress(&self, run_id: &str, progress: Progress);
    /// A run ended.
    fn ended(&self, ended: &Ended);
}

/// A new run: the plugin's arguments, the source and environment `run.json` names, and the
/// breakpoints.
#[derive(Clone, Debug)]
pub struct NewRun {
    pub discover: Value,
    pub source: String,
    pub env: Option<String>,
    pub breakpoints: Breakpoints,
}

/// What [`Runs::start`] does.
#[derive(Clone, Debug)]
pub enum Start {
    New(NewRun),
    /// Continue run `run_id`; its breakpoints' views as the config gives them now (a breakpoint
    /// gone from the config keeps the size `run.json` recorded).
    Resume {
        run_id: String,
        specs: BTreeMap<String, RenderSpec>,
    },
}

/// The run's surroundings.
pub struct Setup {
    pub root: PathBuf,
    pub now_unix: u64,
    pub batch: BatchOptions,
    pub full_page: bool,
    pub compare: Option<CompareWith>,
}

/// The services of a run.
#[derive(Clone)]
pub struct Services {
    pub tools: Arc<dyn PluginTools>,
    pub backend: Arc<dyn RenderBackend>,
    pub events: Arc<dyn RunEvents>,
}

/// The run in progress.
struct Active {
    run_id: String,
    root: PathBuf,
    cancel: Arc<Cancel>,
    /// `None` while the pages are found.
    run: Option<Arc<Mutex<SitemapRun>>>,
    parallel: usize,
}

/// The runs of one window.
#[derive(Default)]
pub struct Runs {
    active: Mutex<Option<Active>>,
    /// The last run started, for a status without an id.
    latest: Mutex<Option<String>>,
}

/// Holds the one-run slot; frees it when dropped.
pub(super) struct Claim {
    pub(super) runs: Arc<Runs>,
}

impl Drop for Claim {
    fn drop(&mut self) {
        lock(&self.runs.active).take();
    }
}

impl Runs {
    /// A registry without runs.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// The run in progress, if any.
    pub fn active_id(&self) -> Option<String> {
        lock(&self.active)
            .as_ref()
            .map(|active| active.run_id.clone())
    }

    /// Starts (or resumes) a run: finds the pages, writes `run.json` and answers; the capture
    /// goes on in a task of its own.
    ///
    /// # Errors
    ///
    /// Another run is in progress, the run to resume is not there, the plugin failed, no page
    /// was found, or the folder cannot be written.
    pub async fn start(
        self: &Arc<Self>,
        setup: Setup,
        start: Start,
        services: Services,
    ) -> Result<CaptureStarted, RunError> {
        let cancel = Arc::new(Cancel::default());
        let run_id = match &start {
            Start::New(_) => {
                let root = setup.root.clone();
                let now = setup.now_unix;
                blocking(move || fresh_id(&root, now)).await??
            }
            Start::Resume { run_id, .. } => run_id.clone(),
        };
        let claim = {
            let mut active = lock(&self.active);
            if active.is_some() {
                return Err(RunError::Busy);
            }
            *active = Some(Active {
                run_id: run_id.clone(),
                root: setup.root.clone(),
                cancel: cancel.clone(),
                run: None,
                parallel: setup.batch.parallel,
            });
            Claim { runs: self.clone() }
        };
        *lock(&self.latest) = Some(run_id.clone());
        let (store, mut run, specs) = prepare(&setup, &run_id, start, &services).await?;
        run.state = RunState::Running;
        run.finished_unix = None;
        let tasks = run.tasks(true, MAX_ATTEMPTS);
        let mut batch = Vec::new();
        let mut taken = Vec::new();
        for task in tasks {
            let url = run
                .pages
                .get(task.page)
                .map(|page| page.url.clone())
                .unwrap_or_default();
            if !loadable(&url) {
                run.record(&task, refused_scheme());
                continue;
            }
            batch.push(BatchTask {
                key: taken.len(),
                url,
                spec: spec_for(&run, &specs, &task.breakpoint),
            });
            taken.push(task);
        }
        let saved = run.clone();
        let saving = store.clone();
        blocking(move || saving.save(&saved)).await??;
        let run = Arc::new(Mutex::new(run));
        if let Some(active) = lock(&self.active).as_mut() {
            active.run = Some(run.clone());
        }
        let started = CaptureStarted {
            run_id: run_id.clone(),
            folder: store.dir().display().to_string(),
            pending: batch.len(),
        };
        let job = Job {
            run_id,
            run,
            store,
            tasks: taken,
            batch,
            setup,
            services,
            cancel,
            claim,
        };
        tokio::spawn(job.execute());
        Ok(started)
    }

    /// Cancels the run `run_id` (the one in progress when `None`): whether it was running.
    ///
    /// # Errors
    ///
    /// No run is in progress and none was named, or the named one is not there.
    pub fn cancel(&self, root: &Path, run_id: Option<&str>) -> Result<(String, bool), RunError> {
        if let Some(active) = lock(&self.active).as_ref()
            && run_id.is_none_or(|id| id == active.run_id)
        {
            active.cancel.cancel();
            return Ok((active.run_id.clone(), true));
        }
        let id = run_id.ok_or(RunError::NotFound(String::new()))?;
        match RunStore::open(root, id, false)? {
            Some(_) => Ok((id.to_owned(), false)),
            None => Err(RunError::NotFound(id.to_owned())),
        }
    }

    /// The run `run_id` (the latest when `None`) as it stands: the one in progress from memory,
    /// any other from its `run.json`.
    ///
    /// # Errors
    ///
    /// There is no such run, or its `run.json` cannot be read.
    pub fn status(
        &self,
        root: &Path,
        run_id: Option<&str>,
    ) -> Result<super::tool::StatusAnswer, RunError> {
        let live = lock(&self.active).as_ref().and_then(|active| {
            let wanted = run_id.is_none_or(|id| id == active.run_id) && active.root == root;
            let run = active.run.as_ref()?;
            wanted.then(|| (lock(run).clone(), active.parallel))
        });
        if let Some((run, parallel)) = live {
            let store = RunStore::open(root, &run.run_id, false)?
                .ok_or_else(|| RunError::NotFound(run.run_id.clone()))?;
            return Ok(status_of(&run, store.dir(), parallel));
        }
        let id = match run_id {
            Some(id) => id.to_owned(),
            None => match lock(&self.latest).clone() {
                Some(id) => id,
                None => rb_visual::sitemap_store::run_ids(root)?
                    .pop()
                    .ok_or(RunError::NotFound(String::new()))?,
            },
        };
        let store =
            RunStore::open(root, &id, false)?.ok_or_else(|| RunError::NotFound(id.clone()))?;
        let run = store
            .load()?
            .ok_or_else(|| RunError::NotFound(id.clone()))?;
        Ok(status_of(&run, store.dir(), 1))
    }
}

impl From<PlanError> for RunError {
    fn from(error: PlanError) -> Self {
        Self::Plan(error)
    }
}

#[cfg(test)]
#[path = "runs_tests.rs"]
mod tests;
