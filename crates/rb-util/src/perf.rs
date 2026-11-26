//! Timing counters (RB-090-32): how long the sync hub, the plugin host and the shell IPC take,
//! kept in release builds so a measurement needs no debug build. Each counter is a named series
//! of durations; its [`Summary`] gives the count, the median, the 95th percentile, the slowest
//! and the mean of the last [`WINDOW`] samples. The app reads them for the diagnostics ZIP
//! (`perf.json`) and the MCP tool `app_perf`; the setting `performance.counters` switches the
//! recording off ([`Registry::set_enabled`]).
//!
//! The counters of the app are [`names`]. Recording takes one short lock; nothing is logged.
//! A [`Gauge`] is a level instead (bytes waiting in a queue, RB-099-19), lock-free and read by
//! its owner.

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::lock;

/// The samples a summary covers, per counter.
pub const WINDOW: usize = 1024;

/// The counters of the app, one list.
pub mod names {
    /// A sync event routed by the hub until its replay starts in the target view's queue.
    pub const SYNC_WAIT: &str = "sync.wait";
    /// A sync event routed by the hub until its replay in a Chromium view answered (K1).
    pub const SYNC_REPLAY_CHROMIUM: &str = "sync.replay.chromium";
    /// The same into a Firefox window (K11).
    pub const SYNC_REPLAY_FIREFOX: &str = "sync.replay.firefox";
    /// The same into a WebKit window (K11).
    pub const SYNC_REPLAY_WEBKIT: &str = "sync.replay.webkit";
    /// The same into an Android device's Chrome.
    pub const SYNC_REPLAY_ANDROID: &str = "sync.replay.android";
    /// The same into Safari (macOS).
    pub const SYNC_REPLAY_SAFARI: &str = "sync.replay.safari";
    /// The same into Mobile Safari on iOS (macOS).
    pub const SYNC_REPLAY_IOS: &str = "sync.replay.ios";
    /// A replay whose engine is not told (a `Replay` that names none).
    pub const SYNC_REPLAY: &str = "sync.replay";
    /// The hub gave a view a URL to load (a follow, a resync) until the view committed it (K11).
    pub const SYNC_NAVIGATION: &str = "sync.navigation";
    /// A plugin call waiting for a free slot of the plugin (`limits.max_concurrent`).
    pub const PLUGIN_WAIT: &str = "plugin.wait";
    /// A plugin call from its slot to its answer.
    pub const PLUGIN_CALL: &str = "plugin.call";
    /// A query of the shell (`cefQuery`) until its answer.
    pub const IPC_QUERY: &str = "ipc.query";
    /// The process start until the first view finished its first load (once per process).
    pub const STARTUP_FIRST_VIEW: &str = "startup.firstView";
}

/// One counter now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    /// The counter, one of [`names`].
    pub name: &'static str,
    /// Samples since the start or the last [`Registry::reset`].
    pub count: u64,
    /// Samples the figures below cover (at most [`WINDOW`]).
    pub window: usize,
    /// Median.
    pub p50: Duration,
    /// 95th percentile.
    pub p95: Duration,
    /// Slowest.
    pub max: Duration,
    /// Mean.
    pub mean: Duration,
}

#[derive(Default)]
struct Series {
    samples: VecDeque<Duration>,
    count: u64,
}

#[derive(Default)]
struct Counters {
    series: BTreeMap<&'static str, Series>,
    once: BTreeMap<&'static str, Duration>,
}

/// The counters of one process ([`global`]); tests make their own.
pub struct Registry {
    enabled: AtomicBool,
    counters: Mutex<Option<Counters>>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    /// An empty registry that records.
    pub const fn new() -> Self {
        Self {
            enabled: AtomicBool::new(true),
            counters: Mutex::new(None),
        }
    }

    /// Whether samples are recorded.
    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Recording on or off; what was recorded stays.
    pub fn set_enabled(&self, on: bool) {
        self.enabled.store(on, Ordering::Relaxed);
    }

    /// One sample of `name`.
    pub fn record(&self, name: &'static str, took: Duration) {
        if !self.enabled() {
            return;
        }
        let mut counters = lock(&self.counters);
        let series = counters
            .get_or_insert_with(Counters::default)
            .series
            .entry(name)
            .or_default();
        if series.samples.len() == WINDOW {
            series.samples.pop_front();
        }
        series.samples.push_back(took);
        series.count += 1;
    }

    /// The one sample of `name` (a start-up time); `false` when it has one already. A reset
    /// keeps it.
    pub fn record_once(&self, name: &'static str, took: Duration) -> bool {
        if !self.enabled() {
            return false;
        }
        let mut counters = lock(&self.counters);
        let once = &mut counters.get_or_insert_with(Counters::default).once;
        if once.contains_key(name) {
            return false;
        }
        once.insert(name, took);
        true
    }

    /// Every counter with a sample, by name.
    pub fn snapshot(&self) -> Vec<Summary> {
        let counters = lock(&self.counters);
        let Some(counters) = counters.as_ref() else {
            return Vec::new();
        };
        let series = counters
            .series
            .iter()
            .map(|(name, series)| summarize(name, series.count, &series.samples));
        let once = counters
            .once
            .iter()
            .map(|(name, took)| summarize(name, 1, &[*took]));
        let mut all: Vec<Summary> = series.chain(once).collect();
        all.sort_by(|a, b| a.name.cmp(b.name));
        all
    }

    /// Forgets every series (a measurement starts afresh); the [`Registry::record_once`]
    /// values stay.
    pub fn reset(&self) {
        if let Some(counters) = lock(&self.counters).as_mut() {
            counters.series.clear();
        }
    }
}

fn summarize<'a>(
    name: &'static str,
    count: u64,
    samples: impl IntoIterator<Item = &'a Duration>,
) -> Summary {
    let mut sorted: Vec<Duration> = samples.into_iter().copied().collect();
    sorted.sort_unstable();
    let total: Duration = sorted.iter().sum();
    let mean = u32::try_from(sorted.len())
        .ok()
        .filter(|len| *len > 0)
        .map_or(Duration::ZERO, |len| total / len);
    Summary {
        name,
        count,
        window: sorted.len(),
        p50: percentile(&sorted, 50.0),
        p95: percentile(&sorted, 95.0),
        max: sorted.last().copied().unwrap_or_default(),
        mean,
    }
}

/// The nearest-rank percentile `p` (0 to 100) of `sorted`; zero for none.
pub fn percentile(sorted: &[Duration], p: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    // At most WINDOW samples: the rank fits any integer type.
    let rank = (p / 100.0 * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

/// A level that goes up and down - the bytes waiting in a queue - with the highest it reached.
/// Lock-free for a hot path; not part of a [`Registry`]'s snapshot, its owner reads it.
#[derive(Debug, Default)]
pub struct Gauge {
    now: AtomicU64,
    peak: AtomicU64,
}

impl Gauge {
    /// A gauge at zero.
    pub const fn new() -> Self {
        Self {
            now: AtomicU64::new(0),
            peak: AtomicU64::new(0),
        }
    }

    /// Raises the level by `amount`; the level after it.
    pub fn add(&self, amount: u64) -> u64 {
        let now = self
            .now
            .fetch_add(amount, Ordering::Relaxed)
            .saturating_add(amount);
        self.peak.fetch_max(now, Ordering::Relaxed);
        now
    }

    /// Lowers the level by `amount`, never below zero.
    pub fn sub(&self, amount: u64) {
        // The closure always answers `Some`: the update cannot fail.
        let _ = self
            .now
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |now| {
                Some(now.saturating_sub(amount))
            });
    }

    /// Puts the level back to zero (what it counted is gone); the peak stays.
    pub fn reset(&self) {
        self.now.store(0, Ordering::Relaxed);
    }

    /// The level now.
    pub fn now(&self) -> u64 {
        self.now.load(Ordering::Relaxed)
    }

    /// The highest level so far.
    pub fn peak(&self) -> u64 {
        self.peak.load(Ordering::Relaxed)
    }
}

/// The registry of this process.
pub fn global() -> &'static Registry {
    static GLOBAL: Registry = Registry::new();
    &GLOBAL
}

/// One sample of `name` in the [`global`] registry.
pub fn record(name: &'static str, took: Duration) {
    global().record(name, took);
}

/// The process start as the app marks it first thing ([`mark_start`]); later calls answer the
/// first mark.
pub fn mark_start() -> Instant {
    static START: OnceLock<Instant> = OnceLock::new();
    *START.get_or_init(Instant::now)
}

#[cfg(test)]
#[path = "perf_tests.rs"]
mod tests;
