//! The replay's debug metric (K1): how long each replay takes from the moment its view's queue
//! hands it over to the last `Input.*` command answered, over the last [`WINDOW`] replays. Every
//! [`LOG_EVERY`] replays it goes to the log at debug level (`replay latency`, p50/p95/max in
//! ms); [`super::CdpReplay::latency`] reads it any time.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

use crate::cdp::lock;

/// The replays the percentiles cover.
pub const WINDOW: usize = 512;
/// Replays between two log lines.
pub const LOG_EVERY: u64 = 100;

/// The percentiles of the last replays.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LatencySummary {
    /// Replays in the window.
    pub samples: usize,
    /// Median, ms.
    pub p50_ms: f64,
    /// 95th percentile, ms.
    pub p95_ms: f64,
    /// Slowest, ms.
    pub max_ms: f64,
}

#[derive(Default)]
struct Window {
    samples: VecDeque<Duration>,
    total: u64,
}

/// The replay times.
#[derive(Default)]
pub struct LatencyStats {
    window: Mutex<Window>,
}

impl LatencyStats {
    /// Records one replay; every [`LOG_EVERY`]th answers the summary to log.
    pub fn record(&self, took: Duration) -> Option<LatencySummary> {
        let mut window = lock(&self.window);
        if window.samples.len() == WINDOW {
            window.samples.pop_front();
        }
        window.samples.push_back(took);
        window.total += 1;
        window
            .total
            .is_multiple_of(LOG_EVERY)
            .then(|| summarize(&window.samples))
    }

    /// The percentiles now.
    pub fn summary(&self) -> LatencySummary {
        summarize(&lock(&self.window).samples)
    }
}

/// The percentiles of `samples`.
pub fn summarize<'a>(samples: impl IntoIterator<Item = &'a Duration>) -> LatencySummary {
    let mut sorted: Vec<Duration> = samples.into_iter().copied().collect();
    sorted.sort_unstable();
    let ms = |duration: Duration| duration.as_secs_f64() * 1000.0;
    LatencySummary {
        samples: sorted.len(),
        p50_ms: ms(percentile(&sorted, 50.0)),
        p95_ms: ms(percentile(&sorted, 95.0)),
        max_ms: ms(sorted.last().copied().unwrap_or_default()),
    }
}

/// The nearest-rank percentile `p` (0 to 100) of `sorted`; zero for none.
pub fn percentile(sorted: &[Duration], p: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    // At most WINDOW (or a test's few hundred) samples: the rank fits any integer type.
    let rank = (p / 100.0 * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

#[cfg(test)]
#[path = "metrics_tests.rs"]
mod tests;
