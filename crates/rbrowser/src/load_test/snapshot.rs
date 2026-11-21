//! What the panel shows of a run (RB-096-07): pushed after every closed second and at the end
//! as the panel event `{"loadTest": <snapshot>}` - elapsed and planned time, users running and
//! wanted, the totals (requests, failures, rate, p50/p95/p99), failures by kind, the latest
//! seconds of the series for the chart, and at the end how it ended and its report's id.

use serde_json::{Value, json};

use super::engine::{End, Tick};
use super::report::Meta;
use super::stats::Recorder;

/// The most seconds of the series a snapshot carries: the chart's width.
pub const SERIES_SHOWN: usize = 300;

/// The snapshot of a run of `meta` at `tick`, `recorder` as it stands; `end` and `report` once
/// it ended.
#[must_use]
pub fn snapshot(
    meta: &Meta,
    recorder: &Recorder,
    tick: &Tick,
    end: Option<End>,
    report: Option<&str>,
) -> Value {
    let series = recorder.series();
    let shown = &series[series.len().saturating_sub(SERIES_SHOWN)..];
    json!({
        "running": end.is_none(),
        "scenario": meta.scenario,
        "name": meta.name,
        "targets": meta.targets.iter().map(|(host, _)| host).collect::<Vec<_>>(),
        "elapsedS": tick.elapsed_s,
        "durationS": meta.profile.duration_s(),
        "users": tick.users,
        "targetUsers": tick.target,
        "maxUsers": meta.profile.users,
        "rpsLimit": meta.rps_limit,
        "totals": recorder.totals(tick.elapsed_s),
        "errors": recorder.errors(),
        "series": shown,
        "end": end.map(End::id),
        "abort": end.and_then(End::abort),
        "reportId": report,
    })
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
