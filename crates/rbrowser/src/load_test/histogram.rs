//! A run's latencies in bounded memory (RB-096-07): a histogram of logarithmic buckets one per
//! cent wide, from 10 microseconds up, so a long run at a high rate keeps a few kilobytes rather
//! than every sample; a percentile is within a per cent of the measured one, never outside the
//! smallest and the largest sample, and the first and last rank are those samples. The seconds and the abort window keep their samples and
//! use `rb_api_model::stats` (exact).

use serde::Serialize;

/// The lowest bucket's bound, in milliseconds.
const FLOOR_MS: f64 = 0.01;
/// The width of a bucket: one per cent.
const GROWTH: f64 = 1.01;
/// The buckets: up to about a hundred days in milliseconds.
const BUCKETS: usize = 3000;

/// The summary of a run's latencies, in milliseconds.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Latency {
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
}

/// The latencies of a run.
#[derive(Clone, Debug)]
pub struct Histogram {
    counts: Vec<u64>,
    count: u64,
    sum: f64,
    min: f64,
    max: f64,
}

impl Default for Histogram {
    fn default() -> Self {
        Self {
            counts: vec![0; BUCKETS],
            count: 0,
            sum: 0.0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
        }
    }
}

fn bucket_of(ms: f64) -> usize {
    if ms <= FLOOR_MS {
        return 0;
    }
    let index = ((ms / FLOOR_MS).ln() / GROWTH.ln()).floor() + 1.0;
    // A finite, positive number of buckets, held below their count.
    (index.max(0.0) as usize).min(BUCKETS - 1)
}

fn middle_of(bucket: usize) -> f64 {
    if bucket == 0 {
        return FLOOR_MS;
    }
    // The bucket covers FLOOR * GROWTH^(b-1) to FLOOR * GROWTH^b.
    let exponent = i32::try_from(bucket).unwrap_or(i32::MAX);
    FLOOR_MS * GROWTH.powi(exponent - 1) * GROWTH.sqrt()
}

impl Histogram {
    /// Adds a sample; one that is no finite number of at least zero is left out.
    pub fn add(&mut self, ms: f64) {
        if !ms.is_finite() || ms < 0.0 {
            return;
        }
        if let Some(slot) = self.counts.get_mut(bucket_of(ms)) {
            *slot += 1;
        }
        self.count += 1;
        self.sum += ms;
        self.min = self.min.min(ms);
        self.max = self.max.max(ms);
    }

    /// Adds every sample of `other`.
    pub fn merge(&mut self, other: &Self) {
        for (mine, theirs) in self.counts.iter_mut().zip(&other.counts) {
            *mine += theirs;
        }
        self.count += other.count;
        self.sum += other.sum;
        self.min = self.min.min(other.min);
        self.max = self.max.max(other.max);
    }

    /// The samples added.
    #[must_use]
    pub const fn count(&self) -> u64 {
        self.count
    }

    /// The `p`th percentile (0 to 100) by nearest rank; none without samples.
    #[must_use]
    pub fn percentile(&self, p: f64) -> Option<f64> {
        if self.count == 0 {
            return None;
        }
        let p = if p.is_nan() { 0.0 } else { p.clamp(0.0, 100.0) };
        // A whole rank in 1..=count.
        let rank = ((p / 100.0) * self.count as f64).ceil().max(1.0) as u64;
        // The first and the last rank are measured values.
        if rank <= 1 {
            return Some(self.min);
        }
        if rank >= self.count {
            return Some(self.max);
        }
        let mut seen = 0;
        for (bucket, count) in self.counts.iter().enumerate() {
            seen += count;
            if seen >= rank {
                return Some(middle_of(bucket).clamp(self.min, self.max));
            }
        }
        Some(self.max)
    }

    /// p50, p95, p99, min, max and mean; none without samples.
    #[must_use]
    pub fn latency(&self) -> Option<Latency> {
        Some(Latency {
            p50: self.percentile(50.0)?,
            p95: self.percentile(95.0)?,
            p99: self.percentile(99.0)?,
            min: self.min,
            max: self.max,
            mean: self.sum / self.count as f64,
        })
    }
}

#[cfg(test)]
#[path = "histogram_tests.rs"]
mod tests;
