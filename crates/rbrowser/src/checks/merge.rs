//! One plugin's own run (`findings.report`, [`super::reported`]) merged into the last run the
//! findings panel shows (RB-094-14). Over the same views it replaces only that plugin's
//! findings, failures and skips and keeps every other plugin's, so the panel, its filters and
//! the export (RB-092-05) still hold the whole run; over other views it replaces the run. The
//! progress details follow: that plugin's rows show the new run, the others stay. A run of
//! chosen checks (the panel's "Run again", [`super::selection`], RB-094-17) merges the same way,
//! each of its plugins' parts replaced.

use std::collections::{BTreeMap, BTreeSet};

use rb_ipc::{CheckJob, CheckJobState, ChecksProgress, ViewId};
use rb_plugin_types::{FindingsFile, FindingsSummary, SKIP_ENGINE};

use super::progress::{SKIP_ENGINE_KEY, engine_label};

/// `previous` with `plugin`'s part replaced by `next` (a run of `plugin` alone); `None` when the
/// two runs checked other views, and `next` replaces `previous` as it is.
///
/// The merged run takes `next`'s id and start (it is the newest state); a finding `plugin`
/// shared with another plugin keeps that plugin's report.
#[must_use]
pub fn plugin_run(
    previous: &FindingsFile,
    next: &FindingsFile,
    plugin: &str,
) -> Option<FindingsFile> {
    plugins_run(previous, next, &[plugin.to_owned()])
}

/// `previous` with the part of every plugin in `replaced` replaced by `next` (a run of those
/// plugins, RB-094-17); `None` when the two runs checked other views. As [`plugin_run`]: `next`'s
/// id and start, a shared finding keeps the plugins not replaced.
#[must_use]
pub fn plugins_run(
    previous: &FindingsFile,
    next: &FindingsFile,
    replaced: &[String],
) -> Option<FindingsFile> {
    let ids = |file: &FindingsFile| -> BTreeSet<u32> { file.views.iter().map(|v| v.id).collect() };
    if ids(previous) != ids(next) {
        return None;
    }
    let mut findings: Vec<_> = previous
        .findings
        .iter()
        .filter_map(|finding| {
            let mut kept = finding.clone();
            kept.plugins.retain(|id| !replaced.contains(id));
            (!kept.plugins.is_empty()).then_some(kept)
        })
        .collect();
    findings.extend(next.findings.iter().cloned());
    let mut plugins = previous.plugins.clone();
    for id in &next.plugins {
        if !plugins.contains(id) {
            plugins.push(id.clone());
        }
    }
    let others = |of: &String| !replaced.contains(of);
    let mut failures: Vec<_> = previous
        .failures
        .iter()
        .filter(|failure| others(&failure.plugin))
        .cloned()
        .collect();
    failures.extend(next.failures.iter().cloned());
    let mut skipped: Vec<_> = previous
        .skipped
        .iter()
        .filter(|skip| others(&skip.plugin))
        .cloned()
        .collect();
    skipped.extend(next.skipped.iter().cloned());
    Some(FindingsFile {
        version: next.version,
        run_id: next.run_id.clone(),
        started_at: next.started_at,
        target: previous.target,
        url: previous.url.clone(),
        plugins,
        views: next.views.clone(),
        summary: FindingsSummary::of(&findings),
        findings,
        failures,
        skipped,
    })
}

/// The progress details after `plugin`'s own run `file`: `previous` (the last run's, finished)
/// with that plugin's rows replaced in place - or added at the end - when the run was merged;
/// with no `previous` (the run replaced the last one) the rows of `file` alone, finished.
#[must_use]
pub fn plugin_progress(
    previous: Option<&ChecksProgress>,
    file: &FindingsFile,
    plugin: &str,
) -> ChecksProgress {
    let rows = plugin_jobs(file, plugin);
    let Some(previous) = previous else {
        return ChecksProgress {
            run_id: file.run_id.clone(),
            elapsed_ms: 0,
            estimate_ms: None,
            jobs: rows,
            finished: true,
            cancelled: false,
        };
    };
    ChecksProgress {
        jobs: replace_rows(&previous.jobs, rows, &[plugin.to_owned()]),
        finished: true,
        ..previous.clone()
    }
}

/// `previous` with the rows of every plugin in `replaced` replaced by its rows of `rows`, where
/// its first row stood; a plugin `previous` has no row of gets its rows at the end, in their
/// order (RB-094-14, RB-094-17).
#[must_use]
pub fn replace_rows(
    previous: &[CheckJob],
    rows: Vec<CheckJob>,
    replaced: &[String],
) -> Vec<CheckJob> {
    let mut placed: BTreeSet<&str> = BTreeSet::new();
    let mut jobs = Vec::with_capacity(previous.len() + rows.len());
    for job in previous {
        if !replaced.contains(&job.plugin) {
            jobs.push(job.clone());
        } else if placed.insert(job.plugin.as_str()) {
            jobs.extend(rows.iter().filter(|row| row.plugin == job.plugin).cloned());
        }
    }
    jobs.extend(
        rows.into_iter()
            .filter(|row| !placed.contains(row.plugin.as_str())),
    );
    jobs
}

/// `plugin`'s rows of `file`: a failure of the plugin as a whole is its one row; otherwise one
/// per view - failed, skipped, or done with the findings in that view - and one for a run
/// without views.
fn plugin_jobs(file: &FindingsFile, plugin: &str) -> Vec<CheckJob> {
    let row = |view: Option<u32>, state: CheckJobState| CheckJob {
        plugin: plugin.to_owned(),
        view: view.map(ViewId),
        breakpoint: view.and_then(|view| {
            file.views
                .iter()
                .find(|checked| checked.id == view)
                .and_then(|checked| checked.breakpoint.clone())
        }),
        state,
        findings: None,
        reason: None,
        reason_params: BTreeMap::new(),
        elapsed_ms: None,
    };
    let failed = |view: Option<u32>| {
        file.failures
            .iter()
            .find(|failure| failure.plugin == plugin && failure.view == view)
            .map(|failure| CheckJob {
                reason: failure.code.clone(),
                reason_params: failure.params.clone(),
                ..row(view, CheckJobState::Failed)
            })
    };
    if let Some(whole) = failed(None) {
        return vec![whole];
    }
    let views: Vec<Option<u32>> = if file.views.is_empty() {
        vec![None]
    } else {
        file.views.iter().map(|checked| Some(checked.id)).collect()
    };
    views
        .into_iter()
        .map(|view| {
            if let Some(job) = failed(view) {
                return job;
            }
            if let Some(skip) = file
                .skipped
                .iter()
                .find(|skip| skip.plugin == plugin && skip.view == view)
            {
                let reason = if skip.reason == SKIP_ENGINE {
                    SKIP_ENGINE_KEY.to_owned()
                } else {
                    format!("checks.skip.{}", skip.reason)
                };
                return CheckJob {
                    reason: Some(reason),
                    reason_params: skip
                        .engine
                        .iter()
                        .map(|engine| ("engine".to_owned(), engine_label(engine)))
                        .collect(),
                    ..row(view, CheckJobState::Skipped)
                };
            }
            let count = file
                .findings
                .iter()
                .filter(|finding| finding.plugins.iter().any(|id| id == plugin))
                .filter(|finding| view.is_none_or(|view| finding.views.contains(&view)))
                .count();
            CheckJob {
                findings: Some(u32::try_from(count).unwrap_or(u32::MAX)),
                ..row(view, CheckJobState::Done)
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "merge_tests.rs"]
mod tests;
