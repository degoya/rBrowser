//! The report of a run (RB-096-07): its scenario, profile and targets, how it ended, the
//! totals (requests, failures, rate, p50/p95/p99), failures by kind, statuses, each step's
//! share and the series of seconds. Kept as JSON and as a page of its own in
//! `.rbrowser/out/load-test/<id>.json` and `.html` ([`super::report_html`]); listed newest
//! first; two runs compared side by side ([`compare`]). The id is the start in seconds since the
//! epoch and the scenario's name, so the newest sorts last. No link on the way is followed.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rb_plugin_types::{Failure, FailureKind};
use serde_json::{Value, json};

use super::engine::Finished;
use super::error;
use super::gate::Standing;
use super::scenario::Profile;

/// The folder, below the project.
pub const OUT_DIR: &str = ".rbrowser/out/load-test";
/// The most reports listed.
pub const MAX_LISTED: usize = 200;
/// The largest report read back.
pub const MAX_REPORT_BYTES: u64 = 16 * 1024 * 1024;

/// What a report says of its run besides the numbers.
#[derive(Clone, Debug, PartialEq)]
pub struct Meta {
    pub scenario: String,
    pub name: String,
    /// Seconds since the epoch.
    pub started_at: u64,
    pub profile: Profile,
    /// The rate the run was held to.
    pub rps_limit: f64,
    pub environment: Option<String>,
    pub targets: Vec<(String, Standing)>,
    /// Each step's request id.
    pub steps: Vec<String>,
}

/// `seconds` since the epoch as `2026-11-21T14:05:01Z`.
#[must_use]
pub fn iso(seconds: u64) -> String {
    let days = i64::try_from(seconds / 86_400).unwrap_or(0);
    let (year, month, day) = rb_diag::cef::civil_from_days(days);
    let rest = seconds % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// The report id of a run of `meta`.
#[must_use]
pub fn id_of(meta: &Meta) -> String {
    let stem: String = meta
        .scenario
        .trim_end_matches(".json")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(80)
        .collect();
    format!("{}-{stem}", meta.started_at)
}

/// Whether `id` names a report of the folder.
#[must_use]
pub fn is_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 120
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// The report of `finished`, a run of `meta`.
#[must_use]
pub fn build(meta: &Meta, finished: &Finished) -> Value {
    let recorder = &finished.recorder;
    let totals = recorder.totals(finished.elapsed_s);
    let steps: Vec<Value> = meta
        .steps
        .iter()
        .zip(recorder.steps())
        .map(|(request, tally)| {
            json!({
                "request": request,
                "requests": tally.requests,
                "errors": tally.errors,
                "meanMs": tally.mean_ms,
            })
        })
        .collect();
    let statuses: BTreeMap<String, u64> = recorder
        .statuses()
        .iter()
        .map(|(status, count)| (status.to_string(), *count))
        .collect();
    json!({
        "version": 1,
        "id": id_of(meta),
        "scenario": meta.scenario,
        "name": meta.name,
        "startedAt": iso(meta.started_at),
        "elapsedS": finished.elapsed_s,
        "end": finished.end.id(),
        "abort": finished.end.abort(),
        "environment": meta.environment,
        "profile": meta.profile,
        "rpsLimit": meta.rps_limit,
        "targets": meta.targets.iter().map(|(host, standing)| json!({ "host": host, "standing": standing })).collect::<Vec<_>>(),
        "totals": totals,
        "errors": recorder.errors(),
        "statuses": statuses,
        "steps": steps,
        "series": recorder.series(),
    })
}

/// The numbers two reports are compared by.
fn figures(report: &Value) -> Value {
    let totals = &report["totals"];
    let latency = &totals["latency"];
    json!({
        "id": report["id"],
        "name": report["name"],
        "startedAt": report["startedAt"],
        "requests": totals["requests"],
        "rps": totals["rps"],
        "errorRate": totals["errorRate"],
        "p50": latency["p50"],
        "p95": latency["p95"],
        "p99": latency["p99"],
    })
}

/// `a` and `b` side by side, and what changed from `a` to `b` (`b - a`; none where either
/// has no number).
#[must_use]
pub fn compare(a: &Value, b: &Value) -> Value {
    let (first, second) = (figures(a), figures(b));
    let mut delta = serde_json::Map::new();
    for key in ["requests", "rps", "errorRate", "p50", "p95", "p99"] {
        let change = match (first[key].as_f64(), second[key].as_f64()) {
            (Some(x), Some(y)) => json!(y - x),
            _ => Value::Null,
        };
        delta.insert(key.to_owned(), change);
    }
    json!({ "a": first, "b": second, "delta": delta })
}

/// One report of the list.
#[must_use]
pub fn row_of(report: &Value) -> Value {
    let totals = &report["totals"];
    json!({
        "id": report["id"],
        "name": report["name"],
        "scenario": report["scenario"],
        "startedAt": report["startedAt"],
        "end": report["end"],
        "requests": totals["requests"],
        "rps": totals["rps"],
        "errorRate": totals["errorRate"],
        "p95": totals["latency"]["p95"],
    })
}

fn io_failure(name: &str, failure: &io::Error) -> Failure {
    error(FailureKind::Io, name).with_param("reason", failure.kind().to_string())
}

/// The reports of one project.
pub struct Reports {
    root: PathBuf,
}

impl Reports {
    #[must_use]
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    fn dir(&self) -> Result<PathBuf, Failure> {
        let mut path = self.root.clone();
        for step in OUT_DIR.split('/') {
            path.push(step);
            if fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_symlink()) {
                return Err(error(FailureKind::AccessDenied, "link"));
            }
        }
        Ok(path)
    }

    /// Keeps `report` and its page; the report's id.
    ///
    /// # Errors
    ///
    /// A link on the way, a write that failed.
    pub fn save(&self, report: &Value, page: &str) -> Result<String, Failure> {
        let id = report["id"].as_str().unwrap_or_default().to_owned();
        if !is_id(&id) {
            return Err(error(FailureKind::BadData, "bad_id").with_param("id", id));
        }
        let dir = self.dir()?;
        fs::create_dir_all(&dir).map_err(|e| io_failure("not_written", &e))?;
        let text = serde_json::to_string_pretty(report).unwrap_or_default();
        rb_util::fs::write_atomic(&dir.join(format!("{id}.json")), text.as_bytes())
            .map_err(|e| io_failure("not_written", &e))?;
        rb_util::fs::write_atomic(&dir.join(format!("{id}.html")), page.as_bytes())
            .map_err(|e| io_failure("not_written", &e))?;
        Ok(id)
    }

    /// The ids, newest first.
    fn ids(&self) -> Result<Vec<String>, Failure> {
        let dir = self.dir()?;
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut ids: Vec<String> = fs::read_dir(&dir)
            .map_err(|e| io_failure("unreadable", &e))?
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter_map(|name| name.strip_suffix(".json").map(str::to_owned))
            .filter(|id| is_id(id))
            .collect();
        ids.sort_by(|a, b| b.cmp(a));
        ids.truncate(MAX_LISTED);
        Ok(ids)
    }

    /// The report `id`, or the newest without one.
    ///
    /// # Errors
    ///
    /// A bad id, none such (`report_not_found`), a file that does not read.
    pub fn read(&self, id: Option<&str>) -> Result<Value, Failure> {
        let id = match id {
            Some(id) if is_id(id) => id.to_owned(),
            Some(id) => {
                return Err(error(FailureKind::BadData, "bad_id")
                    .with_param("id", rb_util::text::cut(id, 80)));
            }
            None => self
                .ids()?
                .into_iter()
                .next()
                .ok_or_else(|| error(FailureKind::NotFound, "no_reports"))?,
        };
        let path = self.dir()?.join(format!("{id}.json"));
        let missing = || error(FailureKind::NotFound, "report_not_found").with_param("id", &id);
        let meta = fs::symlink_metadata(&path).map_err(|_| missing())?;
        if !meta.is_file() {
            return Err(missing());
        }
        if meta.len() > MAX_REPORT_BYTES {
            return Err(
                error(FailureKind::Limit, "file_too_large").with_param("limit", MAX_REPORT_BYTES)
            );
        }
        let text = fs::read_to_string(&path).map_err(|e| io_failure("unreadable", &e))?;
        serde_json::from_str(&text).map_err(|e| {
            error(FailureKind::BadData, "unreadable")
                .with_param("reason", rb_util::text::cut(&e.to_string(), 200))
        })
    }

    /// Every report's row, newest first; one that does not read is left out.
    ///
    /// # Errors
    ///
    /// A link on the way, a folder that does not read.
    pub fn list(&self) -> Result<Vec<Value>, Failure> {
        Ok(self
            .ids()?
            .iter()
            .filter_map(|id| self.read(Some(id)).ok())
            .map(|report| row_of(&report))
            .collect())
    }
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
