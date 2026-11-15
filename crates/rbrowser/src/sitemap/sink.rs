//! Where each finished shot goes: its PNG below the run's folder, its outcome into the run,
//! with `--compare` its comparison with the visual-regression baseline (the diff image beside
//! the shot), and the progress to whoever follows the run.
//!
//! The disk and the diff are blocking work: they run on a blocking thread, never on an async
//! worker, and the PNG is written and compared outside the run's lock. `run.json` is written at
//! most every [`SAVE_EVERY`] (and by [`RunSink::flush`] when the batch ends, also after a
//! cancel), so a large run does not rewrite its whole state after every shot. A crash between
//! two writes loses at most that much: those shots are taken again on resume, into the same
//! files - none twice in the run's record.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use rb_engine::chromium::batch::{BatchSink, BatchTask, ShotResult};
use rb_visual::sitemap::{Outcome, Progress, RunBreakpoint, ShotCompare, SitemapRun, Task};
use rb_visual::sitemap_store::RunStore;
use rb_visual::{BaselineMeta, BaselineStore, DiffOptions, SlugStrategy, Subject, Viewport, meta};

use crate::cdp::lock;

/// The least time between two writes of `run.json` while the run goes on.
pub const SAVE_EVERY: Duration = Duration::from_secs(1);

/// The comparison with the visual-regression baselines (`--compare`).
pub struct CompareWith {
    pub store: BaselineStore,
    pub strategy: SlugStrategy,
    pub options: DiffOptions,
    /// This machine and build (`dpr`, `viewport`, `url`, `slug` are set per shot).
    pub machine: BaselineMeta,
    pub full_page: bool,
}

/// What follows the run's progress (the shell's state, the registry for `sitemap_status`).
pub type OnProgress = Box<dyn Fn(Progress) + Send + Sync>;

/// The sink of one run.
pub struct RunSink {
    shared: Arc<Shared>,
}

struct Shared {
    run: Arc<Mutex<SitemapRun>>,
    store: RunStore,
    /// The run's tasks by the batch's key.
    tasks: Vec<Task>,
    compare: Option<CompareWith>,
    parallel: usize,
    on_progress: OnProgress,
    /// When `run.json` was written last; `None` before the first write.
    saved: Mutex<Option<Instant>>,
}

/// What a shot needs of its page and breakpoint, read under the lock and used outside it.
struct Target {
    slug: String,
    /// The URL a baseline is named by: the sitemap's, before the environment rewrite.
    url: String,
    breakpoint: Option<RunBreakpoint>,
}

impl RunSink {
    /// The sink of `run`, stored in `store`; `tasks` by the batch's key.
    pub fn new(
        run: Arc<Mutex<SitemapRun>>,
        store: RunStore,
        tasks: Vec<Task>,
        compare: Option<CompareWith>,
        parallel: usize,
        on_progress: OnProgress,
    ) -> Self {
        Self {
            shared: Arc::new(Shared {
                run,
                store,
                tasks,
                compare,
                parallel,
                on_progress,
                saved: Mutex::new(None),
            }),
        }
    }

    /// Writes `run.json` now, on a blocking thread.
    ///
    /// # Errors
    ///
    /// The file could not be written.
    pub async fn flush(&self) -> Result<(), String> {
        let shared = self.shared.clone();
        tokio::task::spawn_blocking(move || shared.save(true))
            .await
            .map_err(|error| error.to_string())?
    }
}

impl Shared {
    fn target(&self, task: &Task) -> Option<Target> {
        let run = lock(&self.run);
        let page = run.pages.get(task.page)?;
        Some(Target {
            slug: page.slug.clone(),
            url: page.sitemap_url.clone().unwrap_or_else(|| page.url.clone()),
            breakpoint: run
                .breakpoints
                .iter()
                .find(|breakpoint| breakpoint.name == task.breakpoint)
                .cloned(),
        })
    }

    /// Writes `run.json` when `force`d or [`SAVE_EVERY`] has passed since the last write.
    fn save(&self, force: bool) -> Result<(), String> {
        let run = lock(&self.run);
        let mut saved = lock(&self.saved);
        if !force && saved.is_some_and(|at| at.elapsed() < SAVE_EVERY) {
            return Ok(());
        }
        self.store.save(&run).map_err(|error| error.to_string())?;
        *saved = Some(Instant::now());
        Ok(())
    }

    /// Compares `png` of `task` with its baseline; the outcome for `run.json`.
    fn compare(&self, task: &Task, target: &Target, png: &[u8]) -> Option<ShotCompare> {
        let with = self.compare.as_ref()?;
        let breakpoint = target.breakpoint.as_ref()?;
        let subject = Subject {
            view: 0,
            label: breakpoint.label.clone(),
            breakpoint: breakpoint.name.clone(),
            slug: rb_visual::slug(&target.url, with.strategy),
            url: meta::public_url(&target.url),
        };
        let viewport = Viewport {
            width: breakpoint.width,
            height: breakpoint.height,
        };
        let width = rb_visual::png_size(png).map_or(0, |(width, _)| width);
        let current = BaselineMeta {
            dpr: meta::pixel_ratio(width, breakpoint.width),
            engine: "chromium".to_owned(),
            url: subject.url.clone(),
            slug: subject.slug.clone(),
            viewport,
            full_page: with.full_page,
            ..with.machine.clone()
        };
        let failed = || {
            Some(ShotCompare {
                status: "failed".to_owned(),
                ..ShotCompare::default()
            })
        };
        let baseline = match with.store.read(&subject.slug, &subject.breakpoint) {
            Ok(baseline) => baseline,
            Err(error) => {
                tracing::warn!(%error, "a baseline could not be read");
                return failed();
            }
        };
        let compared =
            match rb_visual::compare(subject, baseline.as_ref(), png, &current, &with.options) {
                Ok(compared) => compared,
                Err(error) => {
                    tracing::warn!(%error, "a shot could not be compared");
                    return failed();
                }
            };
        let diff_file = compared.diff_png.as_deref().and_then(|diff| {
            let name = format!("{}.diff", task.breakpoint);
            self.store.write_shot(&target.slug, &name, diff).ok()
        });
        let status = serde_json::to_value(compared.comparison.status)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_default();
        Some(ShotCompare {
            status,
            ratio: compared.comparison.ratio,
            diff_file,
        })
    }

    /// Writes, compares and records the shot of batch task `key`; the progress after it.
    fn take(&self, key: usize, result: ShotResult) -> Option<Progress> {
        let task = self.tasks.get(key)?;
        let target = self.target(task)?;
        let ms = u64::try_from(result.elapsed.as_millis()).unwrap_or(u64::MAX);
        let file = result.png.as_ref().map_err(Clone::clone).and_then(|png| {
            self.store
                .write_shot(&target.slug, &task.breakpoint, png)
                .map_err(|error| format!("write: {error}"))
        });
        let compared = match (&file, &result.png) {
            (Ok(_), Ok(png)) => self.compare(task, &target, png),
            _ => None,
        };
        let loaded = result.loaded.unwrap_or_default();
        let progress = {
            let mut run = lock(&self.run);
            run.record(
                task,
                Outcome {
                    status: loaded.status,
                    final_url: loaded.final_url,
                    result: file,
                    ms,
                },
            );
            if let Some(compared) = compared
                && let Some(shot) = run
                    .pages
                    .get_mut(task.page)
                    .and_then(|page| page.shots.get_mut(&task.breakpoint))
            {
                shot.compare = Some(compared);
            }
            run.progress(self.parallel)
        };
        if let Err(error) = self.save(false) {
            tracing::warn!(%error, "run.json could not be written");
        }
        Some(progress)
    }
}

#[async_trait]
impl BatchSink for RunSink {
    async fn finished(&self, task: &BatchTask, result: ShotResult) {
        let shared = self.shared.clone();
        let key = task.key;
        let progress = tokio::task::spawn_blocking(move || shared.take(key, result)).await;
        match progress {
            Ok(Some(progress)) => (self.shared.on_progress)(progress),
            Ok(None) => {}
            Err(error) => tracing::warn!(%error, "a shot could not be stored"),
        }
    }
}

#[cfg(test)]
#[path = "sink_tests.rs"]
mod tests;
