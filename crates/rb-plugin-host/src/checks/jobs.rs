//! The jobs of a run, at most `limit` at once, each told as it starts and as it ends - in the
//! order they end, so a finished plugin is seen before the run is through - and stopped by a
//! [`Cancel`]: the jobs waiting never start, the running ones are dropped.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};

use tokio::task::JoinSet;

use super::progress::Cancel;

/// One job: a boxed future, so jobs of different plugins share a list.
pub(crate) type Job<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// How a job ended.
#[derive(Debug)]
pub(crate) enum Ended<T> {
    /// It answered after `elapsed`.
    Answered(T, Duration),
    /// The run was cancelled before it started or while it ran.
    Cancelled,
    /// It panicked: a fault of the host's side, not an answer of the plugin.
    Panicked,
}

/// Runs `jobs` (by index), at most `limit` at once; `on_start(index)` as one starts,
/// `on_end(index, ended)` as one ends - `Panicked` for a job that panicked. After a cancel every
/// job not yet ended ends `Cancelled`.
pub(crate) async fn run_jobs<T: Send + 'static>(
    limit: usize,
    jobs: Vec<Job<T>>,
    cancel: &Cancel,
    mut on_start: impl FnMut(usize),
    mut on_end: impl FnMut(usize, Ended<T>),
) {
    let mut waiting: VecDeque<(usize, Job<T>)> = jobs.into_iter().enumerate().collect();
    let mut running: BTreeSet<usize> = BTreeSet::new();
    let mut set = JoinSet::new();
    let mut tasks = HashMap::new();
    loop {
        while running.len() < limit.max(1) && !cancel.is_cancelled() {
            let Some((index, job)) = waiting.pop_front() else {
                break;
            };
            running.insert(index);
            on_start(index);
            let task = set.spawn(async move {
                let started = Instant::now();
                let answer = job.await;
                (index, answer, started.elapsed())
            });
            tasks.insert(task.id(), index);
        }
        if running.is_empty() {
            break;
        }
        tokio::select! {
            joined = set.join_next() => match joined {
                Some(Ok((index, answer, elapsed))) => {
                    running.remove(&index);
                    on_end(index, Ended::Answered(answer, elapsed));
                }
                Some(Err(error)) => {
                    tracing::warn!(%error, "a check job ended without an answer");
                    if let Some(index) = tasks.get(&error.id()).copied() {
                        running.remove(&index);
                        let ended = if error.is_panic() { Ended::Panicked } else { Ended::Cancelled };
                        on_end(index, ended);
                    }
                }
                None => break,
            },
            () = cancel.cancelled() => {
                set.abort_all();
                for index in std::mem::take(&mut running) {
                    on_end(index, Ended::Cancelled);
                }
            }
        }
    }
    for (index, _) in waiting {
        on_end(index, Ended::Cancelled);
    }
}

#[cfg(test)]
#[path = "jobs_tests.rs"]
mod tests;
