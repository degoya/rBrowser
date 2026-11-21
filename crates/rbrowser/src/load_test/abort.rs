//! The abort rules of a run (RB-096-07): after each second the last `windowS` seconds are
//! judged - their share of failures against `errorRate`, their p95 against `p95Ms` - once they
//! hold `minRequests` requests; the first rule passed stops the run, and the report says which,
//! with the value and the threshold.

use serde::Serialize;

use super::scenario::AbortRules;
use super::stats::Window;

/// Why a run stopped by itself.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Abort {
    /// `errorRate` or `p95Ms`, as the scenario names the rule.
    pub rule: &'static str,
    pub value: f64,
    pub threshold: f64,
}

/// The rule `window` passes, if any.
#[must_use]
pub fn judge(rules: &AbortRules, window: &Window) -> Option<Abort> {
    if window.requests == 0 || window.requests < u64::from(rules.min_requests) {
        return None;
    }
    if let Some(threshold) = rules.error_rate {
        let value = window.errors as f64 / window.requests as f64;
        if value > threshold {
            return Some(Abort {
                rule: "errorRate",
                value,
                threshold,
            });
        }
    }
    if let (Some(threshold), Some(value)) = (rules.p95_ms, window.p95)
        && value > threshold
    {
        return Some(Abort {
            rule: "p95Ms",
            value,
            threshold,
        });
    }
    None
}

#[cfg(test)]
#[path = "abort_tests.rs"]
mod tests;
