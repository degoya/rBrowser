//! The runs on disk (RB-091-02): every folder `.rbrowser/out/lighthouse/<run>/` whose
//! `run.json` reads, newest first; the runs past the retention; and the comparison of two runs
//! of one breakpoint - their scores and Lighthouse's metrics, each with its change.
//!
//! The list comes from `project.list` and each run's `run.json` (`project-read`); a report path
//! is offered only when it lies in its own run folder, so a summary cannot point the report
//! window elsewhere (the core checks the path again). Metrics come from the summary
//! (`metrics`, since RB-091-02) or, for an older run, from the breakpoint's LHR.

use std::collections::BTreeMap;

use rb_plugin_api::{Failure, FailureKind, PluginHost};
use serde_json::{Map, Value, json};

use crate::run::out_path;
use crate::{bad_args, error};

/// The most runs listed: the newest.
pub const MAX_RUNS: usize = 200;
/// Lighthouse's metrics compared, with their unit.
pub const METRICS: [(&str, &str); 5] = [
    ("first-contentful-paint", "ms"),
    ("largest-contentful-paint", "ms"),
    ("total-blocking-time", "ms"),
    ("cumulative-layout-shift", ""),
    ("speed-index", "ms"),
];

/// One breakpoint of a run.
#[derive(Clone, Debug, PartialEq)]
pub struct Measured {
    pub name: String,
    pub scores: Map<String, Value>,
    pub metrics: Map<String, Value>,
    /// The report below the plugin's out folder, `<run>/<file>.html`.
    pub report: String,
    /// The LHR below the plugin's out folder, when it lies in the run's folder.
    pub lhr: Option<String>,
}

/// One run.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub id: String,
    pub started_at: Option<u64>,
    pub url: Option<String>,
    pub cancelled: bool,
    pub breakpoints: Vec<Measured>,
}

/// Whether `name` is a run id, `YYYYMMDD-HHMMSS`.
#[must_use]
pub fn is_run_id(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() == 15
        && bytes[8] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(at, byte)| at == 8 || byte.is_ascii_digit())
}

/// `path` (project-relative, as `run.json` writes it) below the plugin's out folder, when it is
/// a file of the run `run`'s own folder.
fn in_run(path: &str, run: &str) -> Option<String> {
    let rel = path.strip_prefix(&out_path(""))?;
    let file = rel.strip_prefix(run)?.strip_prefix('/')?;
    let plain = !file.is_empty()
        && !file.starts_with('.')
        && file
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
    plain.then(|| rel.to_owned())
}

fn object(value: &Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

fn read_run(host: &mut dyn PluginHost, id: &str) -> Option<Run> {
    let bytes = host
        .project_read(&out_path(&format!("{id}/run.json")))
        .ok()?;
    let summary: Value = serde_json::from_slice(&bytes).ok()?;
    let breakpoints = summary["breakpoints"]
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(|entry| {
            let report = in_run(entry["report"].as_str()?, id)?;
            report.ends_with(".html").then(|| Measured {
                name: entry["breakpoint"].as_str().unwrap_or_default().to_owned(),
                scores: object(&entry["scores"]),
                metrics: object(&entry["metrics"]),
                report,
                lhr: entry["json"].as_str().and_then(|path| in_run(path, id)),
            })
        })
        .collect();
    Some(Run {
        id: id.to_owned(),
        started_at: summary["startedAt"].as_u64(),
        url: summary["url"].as_str().map(str::to_owned),
        cancelled: summary["cancelled"].as_bool().unwrap_or(false),
        breakpoints,
    })
}

/// The runs on disk, newest first, at most [`MAX_RUNS`].
pub fn list(host: &mut dyn PluginHost) -> Vec<Run> {
    let folder = out_path("");
    let names = host
        .project_list(folder.trim_end_matches('/'))
        .unwrap_or_default();
    let mut ids: Vec<String> = names
        .iter()
        .filter_map(|name| name.strip_suffix('/'))
        .filter(|name| is_run_id(name))
        .map(str::to_owned)
        .collect();
    ids.sort_unstable_by(|a, b| b.cmp(a));
    ids.into_iter()
        .filter_map(|id| read_run(host, &id))
        .take(MAX_RUNS)
        .collect()
}

/// The runs past the newest `keep` (0 keeps every run), oldest last.
pub fn beyond(host: &mut dyn PluginHost, keep: u32) -> Vec<String> {
    if keep == 0 {
        return Vec::new();
    }
    let keep = usize::try_from(keep).unwrap_or(usize::MAX);
    list(host)
        .into_iter()
        .skip(keep)
        .map(|run| run.id)
        .collect()
}

/// The `list` answer: `{ runs: [{ runId, startedAt, url, cancelled, breakpoints: [{ breakpoint,
/// scores, report }] }] }`.
pub fn list_answer(host: &mut dyn PluginHost) -> Value {
    let runs: Vec<Value> = list(host)
        .into_iter()
        .map(|run| {
            let breakpoints: Vec<Value> = run
                .breakpoints
                .iter()
                .map(|entry| {
                    json!({ "breakpoint": entry.name, "scores": entry.scores,
                        "report": entry.report })
                })
                .collect();
            json!({ "runId": run.id, "startedAt": run.started_at, "url": run.url,
                "cancelled": run.cancelled, "breakpoints": breakpoints })
        })
        .collect();
    json!({ "runs": runs })
}

/// The breakpoint of the active view, when it has one.
pub fn active_breakpoint(host: &mut dyn PluginHost) -> Option<String> {
    let active = host.active_view().ok().flatten()?;
    host.views()
        .ok()?
        .into_iter()
        .find(|view| view.id == active)
        .map(|view| view.breakpoint)
        .filter(|breakpoint| !breakpoint.is_empty())
}

/// The metrics of an LHR: each of [`METRICS`] that has a `numericValue`.
#[must_use]
pub fn metrics_of_lhr(lhr: &Value) -> BTreeMap<String, Value> {
    METRICS
        .iter()
        .filter_map(|(id, _)| {
            let value = lhr["audits"][id]["numericValue"].as_f64()?;
            Some(((*id).to_owned(), json!(value)))
        })
        .collect()
}

/// The metrics of `entry`: its summary's, else its LHR's.
fn metrics(host: &mut dyn PluginHost, entry: &Measured) -> Map<String, Value> {
    if !entry.metrics.is_empty() {
        return entry.metrics.clone();
    }
    let Some(lhr) = &entry.lhr else {
        return Map::new();
    };
    host.project_read(&out_path(lhr))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .map(|lhr| metrics_of_lhr(&lhr).into_iter().collect())
        .unwrap_or_default()
}

fn delta(before: &Value, after: &Value) -> Value {
    match (before, after) {
        (Value::Number(a), Value::Number(b)) if a.is_u64() && b.is_u64() => {
            json!(b.as_i64().unwrap_or(0) - a.as_i64().unwrap_or(0))
        }
        (Value::Number(a), Value::Number(b)) => match (a.as_f64(), b.as_f64()) {
            (Some(a), Some(b)) => json!(b - a),
            _ => Value::Null,
        },
        _ => Value::Null,
    }
}

fn no_comparison(breakpoint: &str) -> Failure {
    error(FailureKind::NotFound, "no_comparison").with_param("breakpoint", breakpoint)
}

/// Two runs of `breakpoint` (the active view's, else the newest run's first) side by side: the
/// runs named, else the two newest that measured it; the older one is `before`.
///
/// # Errors
///
/// A run id that is none (`errors.bad_args`), or not two runs with the breakpoint
/// (`errors.no_comparison`).
pub fn compare(
    host: &mut dyn PluginHost,
    breakpoint: Option<&str>,
    runs: Option<&[String]>,
) -> Result<Value, Failure> {
    if let Some(runs) = runs
        && (runs.len() != 2 || !runs.iter().all(|run| is_run_id(run)))
    {
        return Err(bad_args("runs: two run ids, YYYYMMDD-HHMMSS"));
    }
    let all = list(host);
    let name = match breakpoint {
        Some(name) => name.to_owned(),
        None => active_breakpoint(host)
            .or_else(|| {
                all.first()?
                    .breakpoints
                    .first()
                    .map(|entry| entry.name.clone())
            })
            .ok_or_else(|| no_comparison(""))?,
    };
    let with: Vec<(&Run, &Measured)> = all
        .iter()
        .filter(|run| runs.is_none_or(|named| named.contains(&run.id)))
        .filter_map(|run| Some((run, run.breakpoints.iter().find(|e| e.name == name)?)))
        .take(2)
        .collect();
    let (Some(&(after_run, after)), Some(&(before_run, before))) = (with.first(), with.get(1))
    else {
        return Err(no_comparison(&name));
    };
    let (before_metrics, after_metrics) = (metrics(host, before), metrics(host, after));
    let mut categories: Vec<&String> = before.scores.keys().chain(after.scores.keys()).collect();
    categories.sort_unstable();
    categories.dedup();
    let scores: Vec<Value> = categories
        .into_iter()
        .map(|category| {
            let a = before.scores.get(category).unwrap_or(&Value::Null);
            let b = after.scores.get(category).unwrap_or(&Value::Null);
            json!({ "category": category, "before": a, "after": b, "delta": delta(a, b) })
        })
        .collect();
    let metrics: Vec<Value> = METRICS
        .iter()
        .map(|(id, unit)| {
            let a = before_metrics.get(*id).cloned().unwrap_or(Value::Null);
            let b = after_metrics.get(*id).cloned().unwrap_or(Value::Null);
            json!({ "metric": id, "unit": unit, "delta": delta(&a, &b), "before": a, "after": b })
        })
        .collect();
    Ok(json!({
        "breakpoint": name, "before": before_run.id, "after": after_run.id,
        "reports": { "before": before.report, "after": after.report },
        "scores": scores, "metrics": metrics,
    }))
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
