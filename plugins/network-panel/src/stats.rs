//! The numbers of an endpoint (RB-096-06): the median and the 95th percentile (nearest rank) of
//! durations and waiting times. Pure; values that are no finite number are left out.

use serde_json::{Value, json};

/// `values`' finite numbers, ascending.
#[must_use]
pub fn sorted(values: impl IntoIterator<Item = f64>) -> Vec<f64> {
    let mut out: Vec<f64> = values
        .into_iter()
        .filter(|value| value.is_finite())
        .collect();
    out.sort_by(f64::total_cmp);
    out
}

/// The median of ascending `values`; `None` for none.
#[must_use]
pub fn median(values: &[f64]) -> Option<f64> {
    let middle = values.len() / 2;
    match values.len() {
        0 => None,
        n if n % 2 == 1 => values.get(middle).copied(),
        _ => Some((values.get(middle - 1)? + values.get(middle)?) / 2.0),
    }
}

/// The `p`th percentile of ascending `values` by nearest rank: the smallest value at least `p`
/// percent of the values are at or below; `None` for none.
#[must_use]
pub fn percentile(values: &[f64], p: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let count = values.len() as f64;
    // `p * count` first: 95 * 20 / 100 is 19 exactly, 0.95 * 20 is not. The rank lies between 1
    // and the count, a few hundred.
    let rank = ((p * count / 100.0).ceil() as usize).clamp(1, values.len());
    values.get(rank - 1).copied()
}

/// One decimal: what an answer carries of a time.
#[must_use]
pub fn round(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

/// The median and the 95th percentile of some times, rounded to one decimal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spread {
    pub median: f64,
    pub p95: f64,
}

impl Spread {
    /// The spread of `values`; `None` without a finite one.
    #[must_use]
    pub fn of(values: impl IntoIterator<Item = f64>) -> Option<Self> {
        let values = sorted(values);
        Some(Self {
            median: round(median(&values)?),
            p95: round(percentile(&values, 95.0)?),
        })
    }

    /// `{"median", "p95"}`.
    #[must_use]
    pub fn json(self) -> Value {
        json!({ "median": self.median, "p95": self.p95 })
    }
}

#[cfg(test)]
#[path = "stats_tests.rs"]
mod tests;
