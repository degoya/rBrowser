//! The halves of a run around [`super::runs::Runs::start`]: [`prepare`] finds the pages (or
//! reads `run.json` back) before the answer, [`Job`] takes the shots in the background and
//! writes the final `run.json` and the gallery.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rb_engine::Emulation;
use rb_engine::chromium::batch::{BatchTask, Cancel, run_batch};
use rb_engine::chromium::hidden::{RenderPool, RenderSpec};
use rb_visual::sitemap::{Outcome, RunState, SitemapRun, Task};
use rb_visual::sitemap_store::{GALLERY_FILE, RunStore};
use serde_json::Value;

use super::backend::DEFAULT_SESSION;
use super::runs::{Claim, Ended, Services, Setup, Start};
use super::sink::RunSink;
use super::status::RunError;
use crate::cdp::lock;

pub(super) async fn blocking<T: Send + 'static>(
    job: impl FnOnce() -> T + Send + 'static,
) -> Result<T, RunError> {
    tokio::task::spawn_blocking(job)
        .await
        .map_err(|error| RunError::Failed(error.to_string()))
}

/// A run id from `now_unix` not taken below `root` (`-2`, `-3`, ... within one second).
pub(super) fn fresh_id(root: &Path, now_unix: u64) -> Result<String, RunError> {
    let base = rb_visual::sitemap_store::run_id(&rb_visual::Stamp::from_unix(now_unix));
    for n in 1..100 {
        let id = if n == 1 {
            base.clone()
        } else {
            format!("{base}-{n}")
        };
        if RunStore::open(root, &id, false)?.is_none() {
            return Ok(id);
        }
    }
    Err(RunError::Failed("no free run id".to_owned()))
}

/// The pages of the plugin's `discover` answer: `(url, sitemap url)`.
pub fn pages_of(answer: &Value) -> Vec<(String, Option<String>)> {
    answer
        .get("pages")
        .and_then(Value::as_array)
        .map(|pages| {
            pages
                .iter()
                .filter_map(|page| {
                    let url = page.get("url").and_then(Value::as_str)?;
                    let from = page.get("sitemapUrl").and_then(Value::as_str);
                    Some((url.to_owned(), from.map(str::to_owned)))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The spec of breakpoint `name` of `run`: the config's, else the size `run.json` recorded.
pub(super) fn spec_for(
    run: &SitemapRun,
    specs: &BTreeMap<String, RenderSpec>,
    name: &str,
) -> RenderSpec {
    specs.get(name).cloned().unwrap_or_else(|| {
        let recorded = run
            .breakpoints
            .iter()
            .find(|breakpoint| breakpoint.name == name);
        RenderSpec {
            breakpoint: name.to_owned(),
            emulation: Emulation {
                width: recorded.map_or(1280, |breakpoint| breakpoint.width),
                height: recorded.map_or(800, |breakpoint| breakpoint.height),
                dpr: recorded.map_or(0.0, |breakpoint| breakpoint.dpr),
                ..Emulation::default()
            },
            session: DEFAULT_SESSION.to_owned(),
        }
    })
}

/// Seconds since the Unix epoch.
pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// Whether a page URL may load in a render view: `http` and `https` only.
pub(super) fn loadable(url: &str) -> bool {
    let lower = url.get(..8).unwrap_or(url).to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

pub(super) fn refused_scheme() -> Outcome {
    Outcome {
        status: None,
        final_url: None,
        result: Err("navigate: only http and https pages are captured".to_owned()),
        ms: 0,
    }
}

/// The run's folder, its state and the specs of its breakpoints: found and written new, or read
/// back for a resume.
pub(super) async fn prepare(
    setup: &Setup,
    run_id: &str,
    start: Start,
    services: &Services,
) -> Result<(RunStore, SitemapRun, BTreeMap<String, RenderSpec>), RunError> {
    match start {
        Start::New(new) => {
            let tools = services.tools.clone();
            let args = new.discover.clone();
            let answer = blocking(move || tools.discover(&args))
                .await?
                .map_err(RunError::Discover)?;
            let pages = pages_of(&answer);
            if pages.is_empty() {
                return Err(RunError::NoPages);
            }
            let (root, id) = (setup.root.clone(), run_id.to_owned());
            let store = blocking(move || RunStore::open(&root, &id, true))
                .await??
                .ok_or_else(|| RunError::Failed("the run's folder was not made".to_owned()))?;
            let mut run = SitemapRun::new(
                run_id,
                &new.source,
                new.breakpoints.run.clone(),
                &pages,
                setup.now_unix,
            );
            run.env = new.env;
            run.full_page = setup.full_page;
            Ok((store, run, new.breakpoints.specs))
        }
        Start::Resume { run_id, specs } => {
            let root = setup.root.clone();
            let (store, run) = blocking(move || {
                let store = RunStore::open(&root, &run_id, false)?
                    .ok_or_else(|| RunError::NotFound(run_id.clone()))?;
                let run = store.load()?.ok_or(RunError::NotFound(run_id))?;
                Ok::<_, RunError>((store, run))
            })
            .await??;
            Ok((store, run, specs))
        }
    }
}

/// The background half of a run.
pub(super) struct Job {
    pub(super) run_id: String,
    pub(super) run: Arc<Mutex<SitemapRun>>,
    pub(super) store: RunStore,
    pub(super) tasks: Vec<Task>,
    pub(super) batch: Vec<BatchTask>,
    pub(super) setup: Setup,
    pub(super) services: Services,
    pub(super) cancel: Arc<Cancel>,
    pub(super) claim: Claim,
}

impl Job {
    pub(super) async fn execute(self) {
        let Job {
            run_id,
            run,
            store,
            tasks,
            batch,
            setup,
            services,
            cancel,
            claim,
        } = self;
        let parallel = setup.batch.parallel;
        let events = services.events.clone();
        let follow = run_id.clone();
        let sink = Arc::new(RunSink::new(
            run.clone(),
            store.clone(),
            tasks,
            setup.compare,
            parallel,
            Box::new(move |progress| events.progress(&follow, progress)),
        ));
        let pool = RenderPool::new(services.backend.clone(), parallel);
        let summary = run_batch(pool.clone(), batch, setup.batch, sink.clone(), cancel).await;
        pool.shutdown().await;
        {
            let mut run = lock(&run);
            run.state = if summary.cancelled {
                RunState::Cancelled
            } else {
                RunState::Finished
            };
            run.finished_unix = Some(now_unix());
        }
        if let Err(error) = sink.flush().await {
            tracing::warn!(%error, "run.json could not be written");
        }
        let finished = lock(&run).clone();
        let gallery = write_gallery(&services, &store, &finished).await;
        drop(claim);
        services.events.ended(&Ended {
            run_id,
            state: finished.state,
            progress: finished.progress(parallel),
            folder: store.dir().to_path_buf(),
            gallery,
        });
    }
}

/// The plugin's gallery of `run`, written as `index.html` into the run's folder.
async fn write_gallery(services: &Services, store: &RunStore, run: &SitemapRun) -> Option<PathBuf> {
    let json = serde_json::to_value(run).ok()?;
    let tools = services.tools.clone();
    let store = store.clone();
    let written = blocking(move || {
        let html = tools.gallery(&json)?;
        store
            .write_file(GALLERY_FILE, html.as_bytes())
            .map_err(|error| error.to_string())
    })
    .await;
    let error = match written {
        Ok(Ok(path)) => return Some(path),
        Ok(Err(error)) => error,
        Err(error) => error.to_string(),
    };
    tracing::warn!(%error, "the gallery was not written");
    None
}

#[cfg(test)]
#[path = "job_tests.rs"]
mod tests;
