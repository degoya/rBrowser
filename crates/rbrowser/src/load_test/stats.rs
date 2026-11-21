//! A run's numbers as it goes (RB-096-07): every request's outcome recorded, each second closed
//! into a point of the series (requests, failures, p50/p95/p99 of that second, users), the
//! window the abort rules judge (the last seconds' samples), and the totals - requests,
//! failures by kind, statuses, each step's share and the latency of the whole run
//! ([`super::histogram`]). A request without an answer (refused, timed out) counts as a failure
//! but adds no latency. The percentiles of a second and of the window are
//! `rb_api_model::stats`, exact.

use std::collections::{BTreeMap, VecDeque};

use rb_api_model::stats::{percentile, sorted};
use serde::Serialize;

use super::histogram::{Histogram, Latency};

/// Why a request failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Failed {
    /// An answer 400 to 499.
    Http4xx,
    /// An answer 500 and up.
    Http5xx,
    /// No answer in time.
    Timeout,
    /// No connection: refused, reset, no such host, TLS.
    Connection,
    /// An answer its checks (`# @rb-check`) did not pass.
    Check,
    /// The request never went out: a host not allowed, a variable without a value.
    Blocked,
    /// Any other failure.
    Other,
}

impl Failed {
    /// The kind's id in the report and the panel.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Http4xx => "http4xx",
            Self::Http5xx => "http5xx",
            Self::Timeout => "timeout",
            Self::Connection => "connection",
            Self::Check => "check",
            Self::Blocked => "blocked",
            Self::Other => "other",
        }
    }

    /// The failure an answer's `status` is, if any.
    #[must_use]
    pub const fn of_status(status: u16) -> Option<Self> {
        match status {
            400..=499 => Some(Self::Http4xx),
            500.. => Some(Self::Http5xx),
            _ => None,
        }
    }
}

/// One request's outcome.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// The step's index.
    pub step: usize,
    /// How long it took, in milliseconds.
    pub ms: f64,
    /// The answer's status, when there was one.
    pub status: Option<u16>,
    pub failed: Option<Failed>,
}

/// One second of the run.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Point {
    /// The second, counted from the start (1 is the first).
    pub t: u32,
    pub requests: u32,
    pub errors: u32,
    pub users: u32,
    pub p50: Option<f64>,
    pub p95: Option<f64>,
    pub p99: Option<f64>,
}

/// The last seconds, as the abort rules see them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Window {
    pub requests: u64,
    pub errors: u64,
    pub p95: Option<f64>,
}

/// The run's totals.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub requests: u64,
    pub errors: u64,
    pub error_rate: f64,
    pub rps: f64,
    pub latency: Option<Latency>,
}

/// One step's share.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepTally {
    pub requests: u64,
    pub errors: u64,
    pub mean_ms: f64,
    #[serde(skip)]
    sum_ms: f64,
    #[serde(skip)]
    timed: u64,
}

#[derive(Default)]
struct Second {
    requests: u32,
    errors: u32,
    samples: Vec<f64>,
}

/// The numbers of one run.
pub struct Recorder {
    current: Second,
    window: VecDeque<Second>,
    window_s: usize,
    histogram: Histogram,
    requests: u64,
    errors: u64,
    by_kind: BTreeMap<&'static str, u64>,
    statuses: BTreeMap<u16, u64>,
    steps: Vec<StepTally>,
    series: Vec<Point>,
}

impl Recorder {
    /// A recorder for `steps` steps whose window is `window_s` seconds.
    #[must_use]
    pub fn new(steps: usize, window_s: u32) -> Self {
        Self {
            current: Second::default(),
            window: VecDeque::new(),
            window_s: usize::try_from(window_s.max(1)).unwrap_or(1),
            histogram: Histogram::default(),
            requests: 0,
            errors: 0,
            by_kind: BTreeMap::new(),
            statuses: BTreeMap::new(),
            steps: vec![StepTally::default(); steps.max(1)],
            series: Vec::new(),
        }
    }

    /// Notes `hit`.
    pub fn record(&mut self, hit: &Hit) {
        self.requests += 1;
        self.current.requests = self.current.requests.saturating_add(1);
        if let Some(kind) = hit.failed {
            self.errors += 1;
            self.current.errors = self.current.errors.saturating_add(1);
            *self.by_kind.entry(kind.id()).or_default() += 1;
        }
        if let Some(status) = hit.status {
            *self.statuses.entry(status).or_default() += 1;
            self.current.samples.push(hit.ms);
            self.histogram.add(hit.ms);
        }
        if let Some(step) = self.steps.get_mut(hit.step) {
            step.requests += 1;
            if hit.failed.is_some() {
                step.errors += 1;
            }
            if hit.status.is_some() {
                step.sum_ms += hit.ms;
                step.timed += 1;
                step.mean_ms = step.sum_ms / step.timed as f64;
            }
        }
    }

    /// The requests of the second not closed yet.
    #[must_use]
    pub const fn pending(&self) -> u32 {
        self.current.requests
    }

    /// Closes second `t` with `users` running: its point, kept in the series.
    pub fn tick(&mut self, t: u32, users: u32) -> Point {
        let second = std::mem::take(&mut self.current);
        let ordered = sorted(&second.samples);
        let point = Point {
            t,
            requests: second.requests,
            errors: second.errors,
            users,
            p50: percentile(&ordered, 50.0),
            p95: percentile(&ordered, 95.0),
            p99: percentile(&ordered, 99.0),
        };
        self.series.push(point);
        self.window.push_back(second);
        while self.window.len() > self.window_s {
            self.window.pop_front();
        }
        point
    }

    /// The closed seconds of the window.
    #[must_use]
    pub fn window(&self) -> Window {
        let samples: Vec<f64> = self
            .window
            .iter()
            .flat_map(|second| second.samples.iter().copied())
            .collect();
        Window {
            requests: self.window.iter().map(|s| u64::from(s.requests)).sum(),
            errors: self.window.iter().map(|s| u64::from(s.errors)).sum(),
            p95: percentile(&sorted(&samples), 95.0),
        }
    }

    /// The totals after `elapsed_s` seconds.
    #[must_use]
    pub fn totals(&self, elapsed_s: f64) -> Totals {
        let share = |part: u64| {
            if self.requests == 0 {
                0.0
            } else {
                part as f64 / self.requests as f64
            }
        };
        Totals {
            requests: self.requests,
            errors: self.errors,
            error_rate: share(self.errors),
            rps: if elapsed_s > 0.0 {
                self.requests as f64 / elapsed_s
            } else {
                0.0
            },
            latency: self.histogram.latency(),
        }
    }

    /// The failures by kind id.
    #[must_use]
    pub const fn errors(&self) -> &BTreeMap<&'static str, u64> {
        &self.by_kind
    }

    /// The answers by status.
    #[must_use]
    pub const fn statuses(&self) -> &BTreeMap<u16, u64> {
        &self.statuses
    }

    /// Each step's share, in the scenario's order.
    #[must_use]
    pub fn steps(&self) -> &[StepTally] {
        &self.steps
    }

    /// The closed seconds.
    #[must_use]
    pub fn series(&self) -> &[Point] {
        &self.series
    }
}

#[cfg(test)]
#[path = "stats_tests.rs"]
mod tests;
