//! A collection run (RB-096-04): every saved request of a folder (all of them without one), one
//! after the other in id order, each delivered as a send is ([`super::deliver`]) and judged by
//! its checks. The [`Report`] is kept as `.rbrowser/out/api-client/report.json` (the project's
//! output, not checked in; no link on the way followed) and its failures become findings
//! ([`super::findings`]). It holds templates, masked URLs, statuses, times and masked verdicts -
//! never a resolved secret. A request without checks passes when it got an answer.

use std::fs;
use std::path::{Path, PathBuf};

use rb_api_model::paths;
use rb_api_model::verdict::{self, Verdict};
use rb_plugin_types::{Failure, FailureKind};
use serde::{Deserialize, Serialize};

use super::deliver::{Courier, deliver};
use super::error;

/// The last run's report below the project root.
pub const REPORT_FILE: &str = ".rbrowser/out/api-client/report.json";
/// The most requests one run sends.
pub const MAX_RUN: usize = 200;

/// One request of a run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ran {
    pub id: String,
    pub name: String,
    pub method: String,
    /// The template, as stored.
    pub url: String,
    /// The URL sent last, every secret masked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sent_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_ms: Option<f64>,
    /// Why there was no answer (its parameters masked where they could hold a value).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<Failure>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<Verdict>,
    pub passed: bool,
}

/// How a run went, counted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tally {
    pub requests: usize,
    pub passed: usize,
    /// Answered, but a check failed.
    pub failed: usize,
    /// No answer.
    pub errors: usize,
    pub checks: usize,
    pub checks_failed: usize,
}

/// A run's report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// Seconds since the epoch.
    pub at: u64,
    /// The folder run; `None` for every request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    pub tally: Tally,
    pub requests: Vec<Ran>,
    /// More requests were in the folder than one run sends ([`MAX_RUN`]).
    #[serde(default)]
    pub cut: bool,
}

impl Tally {
    fn of(requests: &[Ran]) -> Self {
        let mut tally = Self {
            requests: requests.len(),
            ..Self::default()
        };
        for ran in requests {
            if ran.error.is_some() {
                tally.errors += 1;
            } else if ran.passed {
                tally.passed += 1;
            } else {
                tally.failed += 1;
            }
            tally.checks += ran.checks.len();
            tally.checks_failed += ran.checks.iter().filter(|v| !v.passed).count();
        }
        tally
    }
}

/// Whether request `id` belongs to `folder` (every request for none).
fn in_folder(id: &str, folder: Option<&str>) -> bool {
    match folder {
        None => true,
        Some(folder) => id
            .strip_prefix(folder)
            .is_some_and(|rest| rest.starts_with('/')),
    }
}

/// Runs the requests of `folder` (every one for `None` or an empty text) in `environment`.
///
/// # Errors
///
/// `bad_folder` for a folder that is no folder name, the store's failure to list.
pub fn run(
    courier: Courier<'_>,
    folder: Option<&str>,
    environment: Option<String>,
) -> Result<Report, Failure> {
    let folder = folder
        .map(|folder| folder.trim().trim_matches('/'))
        .filter(|folder| !folder.is_empty());
    if let Some(folder) = folder
        && !paths::is_folder(folder)
    {
        return Err(error(FailureKind::BadData, "bad_folder")
            .with_param("folder", rb_util::text::cut(folder, 80)));
    }
    let listed: Vec<_> = courier
        .store
        .list()?
        .into_iter()
        .filter(|listed| in_folder(&listed.id, folder))
        .collect();
    let cut = listed.len() > MAX_RUN;
    let mut requests = Vec::new();
    // Every secret of the run: one request's answer may echo another's (review of RB-096-04).
    let mut secrets: Vec<String> = Vec::new();
    for listed in listed.into_iter().take(MAX_RUN) {
        let ran = match courier.store.read(&listed.id) {
            Ok((request, _)) => {
                let sent = deliver(courier, &request, Some(&listed.id), environment.clone());
                let (sent_url, status, total_ms, failure, checks) = match sent {
                    Ok(sent) => {
                        secrets.extend(sent.secrets.iter().cloned());
                        let checks = sent.verdicts(&request);
                        let url = Some(rb_api_model::mask::mask(&sent.prepared.url, &sent.secrets));
                        match sent.result {
                            Ok(exchange) => (
                                url,
                                Some(exchange.status),
                                Some(exchange.phases.total),
                                None,
                                checks,
                            ),
                            Err(failure) => (url, None, None, Some(failure), Vec::new()),
                        }
                    }
                    Err(failure) => (None, None, None, Some(failure), Vec::new()),
                };
                Ran {
                    passed: failure.is_none() && verdict::all_passed(&checks),
                    id: listed.id,
                    name: request.name,
                    method: request.method,
                    url: request.url,
                    sent_url,
                    status,
                    total_ms,
                    error: failure,
                    checks,
                }
            }
            Err(failure) => Ran {
                id: listed.id,
                name: listed.name,
                method: listed.method,
                url: listed.url,
                sent_url: None,
                status: None,
                total_ms: None,
                error: Some(failure),
                checks: Vec::new(),
                passed: false,
            },
        };
        requests.push(ran);
    }
    for ran in &mut requests {
        if let Some(url) = &mut ran.sent_url {
            *url = rb_api_model::mask::mask(url, &secrets);
        }
        for check in &mut ran.checks {
            if let Some(actual) = &mut check.actual {
                *actual = rb_api_model::mask::mask(actual, &secrets);
            }
        }
    }
    Ok(Report {
        at: courier.ctx.now,
        folder: folder.map(str::to_owned),
        environment,
        tally: Tally::of(&requests),
        requests,
        cut,
    })
}

/// The report file's path, when no folder on the way is a link.
fn path_below(root: &Path) -> Option<PathBuf> {
    let mut path = root.to_path_buf();
    for step in REPORT_FILE.split('/') {
        path.push(step);
        if fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return None;
        }
    }
    Some(path)
}

/// Keeps `report` as the last run's.
///
/// # Errors
///
/// A link on the way, or the write failed.
pub fn keep(root: &Path, report: &Report) -> std::io::Result<()> {
    let path = path_below(root).ok_or_else(|| std::io::Error::other("a link on the way"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_vec_pretty(report).map_err(std::io::Error::other)?;
    rb_util::fs::write_atomic(&path, &text)
}

/// The last run's report; none when there is no file or it does not read.
#[must_use]
pub fn last(root: &Path) -> Option<Report> {
    let bytes = fs::read(path_below(root)?).ok()?;
    serde_json::from_slice(&bytes).ok()
}

#[cfg(test)]
#[path = "collection_tests.rs"]
mod tests;
