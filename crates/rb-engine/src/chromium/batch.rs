//! A batch of shots in hidden render views: `parallel` workers take the tasks in order, each
//! leases a view with the task's spec from the [`RenderPool`] (a view stays open for the next
//! page of its breakpoint), waits its turn on the page's host ([`HostGate`]), loads the page,
//! waits as [`WaitSpec`] says and takes the screenshot - retrying a failure up to `retries`
//! times - and hands the outcome to the [`BatchSink`] (which writes the file and `run.json`).
//! [`Cancel`] stops it at any time: a task interrupted is not reported, so a resumed run takes
//! it again, and nothing is reported twice.

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::Notify;

use super::hidden::{Loaded, RenderPool, RenderSpec, WaitSpec, lock};
use crate::EngineError;

/// The most workers.
pub const MAX_PARALLEL: usize = super::hidden::MAX_RENDER_VIEWS;
/// The most retries of one shot.
pub const MAX_RETRIES: u32 = 5;

/// One shot to take.
#[derive(Clone, Debug, PartialEq)]
pub struct BatchTask {
    /// The caller's key (an index into its own list).
    pub key: usize,
    /// The page.
    pub url: String,
    /// The view it is taken in.
    pub spec: RenderSpec,
}

/// How a batch runs.
#[derive(Clone, Debug, PartialEq)]
pub struct BatchOptions {
    /// Shots at once (1 to [`MAX_PARALLEL`]); also the pool's limit.
    pub parallel: usize,
    /// The least time between two page loads on one host.
    pub host_interval: Duration,
    /// Further attempts after a failure (at most [`MAX_RETRIES`]).
    pub retries: u32,
    /// What each page is waited for.
    pub wait: WaitSpec,
    /// The whole page, not the viewport.
    pub full_page: bool,
}

impl Default for BatchOptions {
    fn default() -> Self {
        Self {
            parallel: 4,
            host_interval: Duration::from_millis(250),
            retries: 1,
            wait: WaitSpec::default(),
            full_page: true,
        }
    }
}

/// What one task gave, after its last attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShotResult {
    /// The navigation's outcome, when the page loaded.
    pub loaded: Option<Loaded>,
    /// The PNG, or why there is none: `timeout`, or `<step>: <reason>` (`navigate`, `wait`,
    /// `screenshot`, `open`; `open: timeout` for a view not ready in time).
    pub png: Result<Vec<u8>, String>,
    /// How long the last attempt took.
    pub elapsed: Duration,
    /// Attempts made.
    pub attempts: u32,
}

/// Where the outcomes go, one call per task, in the order they finish.
#[async_trait]
pub trait BatchSink: Send + Sync {
    /// `task` is done (taken or failed for good).
    async fn finished(&self, task: &BatchTask, result: ShotResult);
}

/// Stops a batch: no task starts after it, a running one is interrupted.
#[derive(Default)]
pub struct Cancel {
    flag: AtomicBool,
    notify: Notify,
}

impl Cancel {
    /// Asks the batch to stop.
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    /// Whether [`Cancel::cancel`] was called.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    /// Resolves once cancelled.
    pub async fn cancelled(&self) {
        loop {
            let notified = self.notify.notified();
            if self.is_cancelled() {
                return;
            }
            notified.await;
        }
    }
}

/// Spaces the page loads of each host by `interval`, however many workers there are.
pub struct HostGate {
    interval: Duration,
    next: Mutex<BTreeMap<String, Instant>>,
}

/// The gate's key for `url`: its authority (`www.example.test:8080`, no user info) in lower
/// case; empty when there is none.
#[must_use]
pub fn host_of(url: &str) -> String {
    rb_util::url::authority_of(url)
        .map(str::to_ascii_lowercase)
        .unwrap_or_default()
}

impl HostGate {
    /// A gate with `interval` between two loads of one host.
    #[must_use]
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            next: Mutex::new(BTreeMap::new()),
        }
    }

    /// Waits until `url`'s host may be loaded again and takes that slot.
    pub async fn wait(&self, url: &str) {
        if self.interval.is_zero() {
            return;
        }
        let slot = {
            let mut next = lock(&self.next);
            let now = Instant::now();
            let entry = next.entry(host_of(url)).or_insert(now);
            let slot = (*entry).max(now);
            *entry = slot + self.interval;
            slot
        };
        tokio::time::sleep_until(slot.into()).await;
    }
}

/// How a batch ended.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BatchSummary {
    pub done: usize,
    pub failed: usize,
    /// Tasks not taken (cancelled before or while they ran).
    pub left: usize,
    pub cancelled: bool,
}

fn step(name: &str, error: &EngineError) -> String {
    format!("{name}: {error}")
}

/// One attempt: lease, load, wait, capture; the lease goes back healthy unless a step failed.
async fn attempt(
    pool: &RenderPool,
    task: &BatchTask,
    options: &BatchOptions,
) -> (Option<Loaded>, Result<Vec<u8>, String>) {
    // The open counts against the page's time: a view that never gets ready fails the attempt
    // (and is closed by the pool once it is there).
    let lease = match tokio::time::timeout(options.wait.timeout, pool.acquire(&task.spec)).await {
        Ok(Ok(lease)) => lease,
        Ok(Err(error)) => return (None, Err(step("open", &error))),
        Err(_) => return (None, Err("open: timeout".to_owned())),
    };
    let backend = pool.backend().clone();
    let view = lease.id();
    let steps = async {
        let loaded = backend
            .navigate(view, &task.url)
            .await
            .map_err(|error| (None, step("navigate", &error)))?;
        backend
            .wait(view, &options.wait)
            .await
            .map_err(|error| (Some(loaded.clone()), step("wait", &error)))?;
        let png = backend
            .screenshot(view, options.full_page)
            .await
            .map_err(|error| (Some(loaded.clone()), step("screenshot", &error)))?;
        Ok::<_, (Option<Loaded>, String)>((loaded, png))
    };
    let outcome = match tokio::time::timeout(options.wait.timeout, steps).await {
        Ok(Ok((loaded, png))) => (Some(loaded), Ok(png)),
        Ok(Err((loaded, error))) => (loaded, Err(error)),
        Err(_) => (None, Err("timeout".to_owned())),
    };
    pool.release(lease, outcome.1.is_ok()).await;
    outcome
}

/// Takes `task` with its retries; `None` when cancelled first.
async fn take(
    pool: &RenderPool,
    gate: &HostGate,
    task: &BatchTask,
    options: &BatchOptions,
    cancel: &Cancel,
) -> Option<ShotResult> {
    let tries = options.retries.min(MAX_RETRIES) + 1;
    let mut last = None;
    for attempts in 1..=tries {
        let run = async {
            gate.wait(&task.url).await;
            let started = Instant::now();
            let (loaded, png) = attempt(pool, task, options).await;
            ShotResult {
                loaded,
                png,
                elapsed: started.elapsed(),
                attempts,
            }
        };
        let result = tokio::select! {
            result = run => result,
            () = cancel.cancelled() => return None,
        };
        let ok = result.png.is_ok();
        last = Some(result);
        if ok {
            break;
        }
    }
    last
}

/// Runs `tasks` in order with `options.parallel` workers on `pool`; every finished task goes
/// to `sink`. Returns when all are taken or the batch was cancelled.
pub async fn run_batch(
    pool: Arc<RenderPool>,
    tasks: Vec<BatchTask>,
    options: BatchOptions,
    sink: Arc<dyn BatchSink>,
    cancel: Arc<Cancel>,
) -> BatchSummary {
    let queue = Arc::new(Mutex::new(VecDeque::from(tasks)));
    let gate = Arc::new(HostGate::new(options.host_interval));
    let summary = Arc::new(Mutex::new(BatchSummary::default()));
    let options = Arc::new(options);
    let workers = options.parallel.clamp(1, MAX_PARALLEL);
    let mut set = tokio::task::JoinSet::new();
    for _ in 0..workers {
        let (queue, gate, summary, options, sink, cancel, pool) = (
            queue.clone(),
            gate.clone(),
            summary.clone(),
            options.clone(),
            sink.clone(),
            cancel.clone(),
            pool.clone(),
        );
        set.spawn(async move {
            loop {
                if cancel.is_cancelled() {
                    return;
                }
                let Some(task) = lock(&queue).pop_front() else {
                    return;
                };
                let Some(result) = take(&pool, &gate, &task, &options, &cancel).await else {
                    lock(&queue).push_front(task);
                    return;
                };
                {
                    let mut summary = lock(&summary);
                    if result.png.is_ok() {
                        summary.done += 1;
                    } else {
                        summary.failed += 1;
                    }
                }
                sink.finished(&task, result).await;
            }
        });
    }
    while set.join_next().await.is_some() {}
    let mut summary = *lock(&summary);
    summary.left = lock(&queue).len();
    summary.cancelled = cancel.is_cancelled();
    summary
}

#[cfg(test)]
#[path = "batch_tests.rs"]
mod tests;
