//! The runs of the environment comparison, without the window: [`Runs::start`] writes
//! `report.json` and answers, the shots go on in a task of its own through the sitemap
//! capture's render pool and batch (hidden render views, never the grid), and each breakpoint
//! and path is diffed across every environment pair as soon as its last shot is in, on a
//! blocking thread. When the batch ends `report.json` is written again and `report.html`
//! beside it. One run at a time, cancelled with [`Runs::cancel`] (the batch's [`Cancel`]); the
//! render views and the events are traits, so the whole run is tested against fakes. The shots'
//! sink is [`super::sink`].

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rb_engine::chromium::batch::{BatchOptions, BatchTask, Cancel, run_batch};
use rb_engine::chromium::hidden::{RenderBackend, RenderPool, RenderSpec};
use rb_visual::sitemap::RunState;
use rb_visual::sitemap_store::{RunStore, run_ids_in};
use rb_visual::{DiffOptions, Stamp};

use super::plan::{PlanError, pairs, url_in};
use super::report::{EnvRun, REPORT_HTML, REPORT_JSON, Summary};
use super::sink::Sink;
use super::tool::Started;
use crate::cdp::lock;

/// The runs' folder below the project root.
pub const OUT_DIR: [&str; 3] = [".rbrowser", "out", "env-compare"];
/// The largest `report.json` read.
pub const MAX_REPORT_BYTES: u64 = 64 * 1024 * 1024;

/// Why a run cannot start or be found. The message starts with its stable code.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("env_compare.error.no_project: no project is open")]
    NoProject,
    #[error("env_compare.error.busy: another comparison is running")]
    Busy,
    #[error(transparent)]
    Plan(#[from] PlanError),
    #[error("env_compare.error.not_found: no comparison {0}")]
    NotFound(String),
    #[error("env_compare.error.failed: {0}")]
    Failed(String),
}

impl RunError {
    /// The stable code of the error.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoProject => "env_compare.error.no_project",
            Self::Busy => "env_compare.error.busy",
            Self::Plan(error) => error.code(),
            Self::NotFound(_) => "env_compare.error.not_found",
            Self::Failed(_) => "env_compare.error.failed",
        }
    }
}

impl From<rb_visual::VisualError> for RunError {
    fn from(error: rb_visual::VisualError) -> Self {
        Self::Failed(error.to_string())
    }
}

/// Whoever follows the runs.
pub trait RunEvents: Send + Sync {
    /// A shot of `run_id` is in.
    fn progress(&self, run_id: &str, done: usize, total: usize);
    /// A run ended.
    fn ended(&self, summary: &Summary);
}

/// The texts of `report.html`, by key below `envcompare.report.`.
pub type Texts = Arc<dyn Fn(&str) -> String + Send + Sync>;

/// A new run: the record with its environments, breakpoints and paths, each breakpoint's render
/// spec (by index), the shots and the diff.
pub struct NewRun {
    pub root: PathBuf,
    pub now_unix: u64,
    pub run: EnvRun,
    pub specs: Vec<RenderSpec>,
    pub batch: BatchOptions,
    pub options: DiffOptions,
    pub texts: Texts,
}

/// The services of a run.
#[derive(Clone)]
pub struct Services {
    pub backend: Arc<dyn RenderBackend>,
    pub events: Arc<dyn RunEvents>,
}

struct Active {
    run_id: String,
    root: PathBuf,
    run: Option<Arc<Mutex<EnvRun>>>,
    cancel: Arc<Cancel>,
}

/// The runs of one window.
#[derive(Default)]
pub struct Runs {
    active: Mutex<Option<Active>>,
    latest: Mutex<Option<String>>,
}

/// Holds the one-run slot; frees it when dropped.
struct Claim(Arc<Runs>);

impl Drop for Claim {
    fn drop(&mut self) {
        lock(&self.0.active).take();
    }
}

pub(super) async fn blocking<T: Send + 'static>(
    job: impl FnOnce() -> T + Send + 'static,
) -> Result<T, RunError> {
    tokio::task::spawn_blocking(job)
        .await
        .map_err(|error| RunError::Failed(error.to_string()))
}

/// A run id from `now_unix` not taken below `root`, and its folder.
fn fresh_store(root: &Path, now_unix: u64) -> Result<(String, RunStore), RunError> {
    let base = rb_visual::sitemap_store::run_id(&Stamp::from_unix(now_unix));
    for n in 1..100 {
        let id = if n == 1 {
            base.clone()
        } else {
            format!("{base}-{n}")
        };
        if RunStore::open_in(root, &OUT_DIR, &id, false)?.is_none() {
            let store = RunStore::open_in(root, &OUT_DIR, &id, true)?
                .ok_or_else(|| RunError::Failed("the run's folder was not made".to_owned()))?;
            return Ok((id, store));
        }
    }
    Err(RunError::Failed("no free run id".to_owned()))
}

fn save(store: &RunStore, run: &EnvRun) -> Result<(), RunError> {
    let mut text =
        serde_json::to_string_pretty(run).map_err(|error| RunError::Failed(error.to_string()))?;
    text.push('\n');
    store.write_file(REPORT_JSON, text.as_bytes())?;
    Ok(())
}

impl Runs {
    /// A registry without runs.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Starts a run: its folder and `report.json`, then the shots in the background.
    ///
    /// # Errors
    ///
    /// Another run is in progress, or the folder cannot be written.
    pub async fn start(
        self: &Arc<Self>,
        new: NewRun,
        services: Services,
    ) -> Result<Started, RunError> {
        let cancel = Arc::new(Cancel::default());
        let claim = {
            let mut active = lock(&self.active);
            if active.is_some() {
                return Err(RunError::Busy);
            }
            *active = Some(Active {
                run_id: String::new(),
                root: new.root.clone(),
                run: None,
                cancel: cancel.clone(),
            });
            Claim(self.clone())
        };
        let NewRun {
            root,
            now_unix,
            mut run,
            specs,
            batch,
            options,
            texts,
        } = new;
        let folder = root.clone();
        let (run_id, store) = blocking(move || fresh_store(&folder, now_unix)).await??;
        let pairs = pairs(run.environments.len());
        run.run_id = run_id.clone();
        run.created_unix = now_unix;
        run.state = RunState::Running;
        run.threshold = options.threshold();
        run.full_page = batch.full_page;
        run.fill(&pairs, url_in);
        let tasks: Vec<BatchTask> = run
            .shots
            .iter()
            .enumerate()
            .filter_map(|(key, shot)| {
                Some(BatchTask {
                    key,
                    url: shot.url.clone(),
                    spec: specs.get(shot.breakpoint)?.clone(),
                })
            })
            .collect();
        let (saving, saved) = (store.clone(), run.clone());
        blocking(move || save(&saving, &saved)).await??;
        let started = Started {
            run_id: run_id.clone(),
            folder: store.dir().display().to_string(),
            shots: run.shots.len(),
            diffs: run.diffs.len(),
        };
        let run = Arc::new(Mutex::new(run));
        if let Some(active) = lock(&self.active).as_mut() {
            active.run_id = run_id.clone();
            active.run = Some(run.clone());
        }
        *lock(&self.latest) = Some(run_id.clone());
        let sink = Arc::new(Sink {
            run: run.clone(),
            store: store.clone(),
            groups: Mutex::new(BTreeMap::new()),
            pairs,
            options,
            max_ratio: lock(&run).max_ratio,
            events: services.events.clone(),
        });
        tokio::spawn(async move {
            let pool = RenderPool::new(services.backend.clone(), batch.parallel);
            let summary = run_batch(pool.clone(), tasks, batch, sink, cancel).await;
            pool.shutdown().await;
            let finished = {
                let mut run = lock(&run);
                run.state = if summary.cancelled {
                    RunState::Cancelled
                } else {
                    RunState::Finished
                };
                run.finished_unix = Some(crate::sitemap::runs::now_unix());
                run.clone()
            };
            let (writing, record) = (store.clone(), finished.clone());
            let written = blocking(move || {
                save(&writing, &record)?;
                let html = super::html::render(&record, &*texts);
                Ok::<_, RunError>(writing.write_file(REPORT_HTML, html.as_bytes())?)
            })
            .await;
            let report = match written {
                Ok(Ok(path)) => Some(path.display().to_string()),
                Ok(Err(error)) | Err(error) => {
                    tracing::warn!(%error, "the comparison's report was not written");
                    None
                }
            };
            drop(claim);
            let folder = store.dir().display().to_string();
            services.events.ended(&finished.summary(&folder, report));
        });
        Ok(started)
    }

    /// Cancels the run `run_id` (the one in progress when `None`): its id and whether it was
    /// running. What was taken stays; `report.json` and `report.html` are written as the run
    /// ends.
    ///
    /// # Errors
    ///
    /// No run is in progress and none was named, or the named one is not there.
    pub fn cancel(&self, root: &Path, run_id: Option<&str>) -> Result<(String, bool), RunError> {
        if let Some(active) = lock(&self.active).as_ref()
            && active.root == root
            && run_id.is_none_or(|id| id == active.run_id)
        {
            active.cancel.cancel();
            return Ok((active.run_id.clone(), true));
        }
        let id = run_id.ok_or_else(|| RunError::NotFound(String::new()))?;
        match RunStore::open_in(root, &OUT_DIR, id, false)? {
            Some(_) => Ok((id.to_owned(), false)),
            None => Err(RunError::NotFound(id.to_owned())),
        }
    }

    /// The run `run_id` (the latest when `None`): the one in progress from memory, any other
    /// from its `report.json`.
    ///
    /// # Errors
    ///
    /// There is no such run, or its `report.json` cannot be read.
    pub fn status(&self, root: &Path, run_id: Option<&str>) -> Result<Summary, RunError> {
        let live = lock(&self.active).as_ref().and_then(|active| {
            let wanted = run_id.is_none_or(|id| id == active.run_id) && active.root == root;
            wanted.then(|| active.run.as_ref().map(|run| lock(run).clone()))?
        });
        let id = match (live.as_ref(), run_id) {
            (Some(run), _) => run.run_id.clone(),
            (None, Some(id)) => id.to_owned(),
            (None, None) => match lock(&self.latest).clone() {
                Some(id) => id,
                None => run_ids_in(root, &OUT_DIR)?
                    .pop()
                    .ok_or_else(|| RunError::NotFound(String::new()))?,
            },
        };
        let store = RunStore::open_in(root, &OUT_DIR, &id, false)?
            .ok_or_else(|| RunError::NotFound(id.clone()))?;
        let folder = store.dir().display().to_string();
        if let Some(run) = live {
            return Ok(run.summary(&folder, None));
        }
        let bytes = store
            .read(REPORT_JSON, MAX_REPORT_BYTES)?
            .ok_or_else(|| RunError::NotFound(id.clone()))?;
        let run: EnvRun = serde_json::from_slice(&bytes)
            .map_err(|error| RunError::Failed(format!("{REPORT_JSON}: {error}")))?;
        let report = store
            .read(REPORT_HTML, MAX_REPORT_BYTES)
            .ok()
            .flatten()
            .map(|_| store.dir().join(REPORT_HTML).display().to_string());
        Ok(run.summary(&folder, report))
    }
}

#[cfg(test)]
#[path = "runs_tests.rs"]
mod tests;
