//! Order statistics of timings (RB-096-04): the benchmark's min, median, p95 and max per phase,
//! and the percentiles the load test (RB-096-07) reports. Pure and small: a percentile is the
//! nearest rank of the sorted samples (no interpolation, so it is always a measured value);
//! samples that are no finite number are left out.

use serde::Serialize;

/// The finite samples of `samples`, ascending.
#[must_use]
pub fn sorted(samples: &[f64]) -> Vec<f64> {
    let mut out: Vec<f64> = samples.iter().copied().filter(|s| s.is_finite()).collect();
    out.sort_by(f64::total_cmp);
    out
}

/// The `p`th percentile (0 to 100, held to that range) of the ascending `sorted` by nearest
/// rank; `None` for no samples.
#[must_use]
pub fn percentile(sorted: &[f64], p: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let p = if p.is_nan() { 0.0 } else { p.clamp(0.0, 100.0) };
    let count = sorted.len() as f64;
    // The rank is in 1..=len: the float is a whole number of at most the sample count.
    let rank = ((p / 100.0) * count).ceil().max(1.0) as usize;
    sorted.get(rank.min(sorted.len()) - 1).copied()
}

/// The order statistics of one series of samples, in its unit (milliseconds for timings).
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub count: usize,
    pub min: f64,
    pub median: f64,
    pub p95: f64,
    pub max: f64,
    pub mean: f64,
}

impl Summary {
    /// The summary of `samples`; `None` when none is a finite number.
    #[must_use]
    pub fn of(samples: &[f64]) -> Option<Self> {
        let sorted = sorted(samples);
        let (&min, &max) = (sorted.first()?, sorted.last()?);
        let mean = sorted.iter().sum::<f64>() / sorted.len() as f64;
        Some(Self {
            count: sorted.len(),
            min,
            median: percentile(&sorted, 50.0)?,
            p95: percentile(&sorted, 95.0)?,
            max,
            mean,
        })
    }
}

#[cfg(test)]
#[path = "stats_tests.rs"]
mod tests;
