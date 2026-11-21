//! Where a comparison's shots go (split from [`super::runs`]): each is written into the run's
//! folder, and once every environment of a breakpoint and path is in, they are diffed across
//! every environment pair on a blocking thread. The engine comparison (RB-094-03) takes the same
//! sink with its engines as the environments.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rb_engine::chromium::batch::{BatchSink, BatchTask, ShotResult};
use rb_visual::DiffOptions;
use rb_visual::sitemap_store::RunStore;

use super::names::{diff_name, shot_name};
use super::report::{EnvRun, diff_group};
use super::runs::{RunEvents, blocking};
use crate::cdp::lock;

/// Each environment's shot of one breakpoint and path: `None` until it is in, `Some(None)`
/// for a failed one.
type Group = Vec<Option<Option<Vec<u8>>>>;

/// Where the shots go: each is written into the run's folder, and once every environment of a
/// breakpoint and path is in, they are diffed.
pub(crate) struct Sink {
    pub(crate) run: Arc<Mutex<EnvRun>>,
    pub(crate) store: RunStore,
    /// `(path, breakpoint)` -> each environment's PNG (`Some(None)` for a failed shot).
    pub(crate) groups: Mutex<BTreeMap<(usize, usize), Group>>,
    pub(crate) pairs: Vec<(usize, usize)>,
    pub(crate) options: DiffOptions,
    pub(crate) max_ratio: f64,
    pub(crate) events: Arc<dyn RunEvents>,
}

impl Sink {
    /// Records the shot `key`; its PNG for the diff, `None` when there is none.
    async fn record(&self, key: usize, result: ShotResult) -> Option<Vec<u8>> {
        let (name, folder) = {
            let run = lock(&self.run);
            let shot = run.shots.get(key)?;
            let environment = run.environments.get(shot.environment)?;
            let breakpoint = run.breakpoints.get(shot.breakpoint)?;
            (
                shot_name(
                    shot.environment,
                    &environment.id,
                    shot.breakpoint,
                    &breakpoint.name,
                ),
                run.paths.get(shot.path)?.folder.clone(),
            )
        };
        let status = result.loaded.as_ref().and_then(|loaded| loaded.status);
        let outcome = match result.png {
            Ok(png) => {
                let store = self.store.clone();
                blocking(move || {
                    let written = store.write_shot(&folder, &name, &png);
                    (written, png)
                })
                .await
                .map_err(|error| error.to_string())
                .and_then(|(written, png)| {
                    written.map(|file| (file, png)).map_err(|e| e.to_string())
                })
            }
            Err(error) => Err(error),
        };
        let (done, total, run_id) = {
            let mut run = lock(&self.run);
            let shot = run.shots.get_mut(key)?;
            shot.status = status;
            match &outcome {
                Ok((file, _)) => shot.file = Some(file.clone()),
                Err(error) => shot.error = Some(error.clone()),
            }
            let done = run.shots.iter().filter(|shot| shot.done()).count();
            (done, run.shots.len(), run.run_id.clone())
        };
        self.events.progress(&run_id, done, total);
        outcome.ok().map(|(_, png)| png)
    }

    /// Diffs the group of `path` and `breakpoint` and records each pair.
    async fn diff(&self, path: usize, breakpoint: usize, shots: Group) {
        let shots: Vec<Option<Vec<u8>>> = shots.into_iter().map(Option::flatten).collect();
        let (folder, bp_name) = {
            let run = lock(&self.run);
            let folder = run.paths.get(path).map(|p| p.folder.clone());
            let name = run.breakpoints.get(breakpoint).map(|b| b.name.clone());
            match (folder, name) {
                (Some(folder), Some(name)) => (folder, name),
                _ => return,
            }
        };
        let (pairs, options, max_ratio, store) = (
            self.pairs.clone(),
            self.options,
            self.max_ratio,
            self.store.clone(),
        );
        let found = blocking(move || {
            diff_group(&shots, &pairs, &options, max_ratio)
                .into_iter()
                .map(|mut found| {
                    let image = found.png.take().and_then(|png| {
                        let name = diff_name(found.a, found.b, breakpoint, &bp_name);
                        store.write_shot(&folder, &name, &png).ok()
                    });
                    (found, image)
                })
                .collect::<Vec<_>>()
        })
        .await;
        let Ok(found) = found else {
            return;
        };
        let mut run = lock(&self.run);
        for (found, image) in found {
            if let Some(diff) = run.diffs.iter_mut().find(|diff| {
                (diff.path, diff.a, diff.b, diff.breakpoint) == (path, found.a, found.b, breakpoint)
            }) {
                diff.status = found.status;
                diff.ratio = found.ratio;
                diff.different_pixels = found.different_pixels;
                diff.total_pixels = found.total_pixels;
                diff.image = image;
                diff.regions = found.regions;
                diff.error = found.error;
            }
        }
    }
}

#[async_trait]
impl BatchSink for Sink {
    async fn finished(&self, task: &BatchTask, result: ShotResult) {
        let place = {
            let run = lock(&self.run);
            run.shots.get(task.key).map(|shot| {
                (
                    shot.path,
                    shot.breakpoint,
                    shot.environment,
                    run.environments.len(),
                )
            })
        };
        let Some((path, breakpoint, environment, count)) = place else {
            return;
        };
        let png = self.record(task.key, result).await;
        let complete = {
            let mut groups = lock(&self.groups);
            let group = groups
                .entry((path, breakpoint))
                .or_insert_with(|| vec![None; count]);
            if let Some(slot) = group.get_mut(environment) {
                *slot = Some(png);
            }
            if group.iter().all(Option::is_some) {
                groups.remove(&(path, breakpoint))
            } else {
                None
            }
        };
        if let Some(shots) = complete {
            self.diff(path, breakpoint, shots).await;
        }
    }
}
