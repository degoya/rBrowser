//! A Lighthouse run over the breakpoints: one target after another (`plan`), each measured by
//! the sidecar (`sidecar`), its reports saved, its scores and findings read (`lhr`).
//!
//! **Output** (capability `out-write`, below `.rbrowser/out/lighthouse/`):
//! `<run>/<breakpoint>.html` and `.json` - Lighthouse's HTML report and JSON result -,
//! `<run>/run.json`, the summary the answer is, and `latest.json`, the same summary with the
//! scores of the run before it (`previous`), which the panel and `lighthouse_status` show. The
//! run id is the start time in UTC, `YYYYMMDD-HHMMSS`. Older runs stay; the contract has no way
//! to remove files.
//!
//! A breakpoint that fails (the page does not load, the report is too large) is listed under
//! `failures` and the run goes on; a cancelled job ends the run with `cancelled: true`.

use rb_plugin_api::{CheckContext, Failure, FailureKind, Finding, PluginHost};
use serde_json::{Value, json};

use crate::plan::{self, Selection, Skipped, Target};
use crate::project::Project;
use crate::sidecar::{self, Ended, MAX_REPORT_BYTES};
use crate::{PLUGIN_ID, error, lhr, settings};

/// The version of `run.json`.
pub const SUMMARY_VERSION: u32 = 1;
/// The summary of the newest run, below the plugin's out folder.
pub const LATEST: &str = "latest.json";

/// What a call asks for.
#[derive(Clone, Debug, Default)]
pub struct Request {
    pub breakpoints: Option<Vec<String>>,
    pub categories: Option<Vec<String>>,
    pub url: Option<String>,
    pub view: Option<u32>,
    /// Report the findings into the findings panel (the command).
    pub report: bool,
}

/// `YYYYMMDD-HHMMSS` of a Unix time, in UTC.
#[must_use]
pub fn run_id(unix_seconds: u64) -> String {
    let days = i64::try_from(unix_seconds / 86_400).unwrap_or(0);
    let rest = unix_seconds % 86_400;
    // Howard Hinnant's civil-from-days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// The project-relative path of `name` in the plugin's out folder.
#[must_use]
pub fn out_path(name: &str) -> String {
    format!(".rbrowser/out/{PLUGIN_ID}/{name}")
}

fn skipped_json(skipped: &[Skipped]) -> Vec<Value> {
    skipped
        .iter()
        .map(|skip| {
            json!({ "breakpoint": skip.breakpoint, "view": skip.view, "engine": skip.engine,
                "reason": skip.reason })
        })
        .collect()
}

/// Measures one target: its report and its LHR text.
fn measure(
    host: &mut dyn PluginHost,
    target: &Target,
    throttling: &str,
    categories: &[String],
) -> Result<Result<(lhr::Report, String, String), String>, Failure> {
    let settings = target.settings(throttling, categories);
    let job = sidecar::start(host, &target.url, &target.breakpoint, &settings)?;
    let ended = sidecar::wait(host, &job, &target.breakpoint);
    let outcome = match ended {
        Ok(Ended::Done {
            json_bytes,
            html_bytes,
            ..
        }) if json_bytes > MAX_REPORT_BYTES || html_bytes > MAX_REPORT_BYTES => {
            Ok(Err("too_large".to_owned()))
        }
        Ok(Ended::Done { .. }) => {
            let json = sidecar::report(host, &job, "json");
            let html = json.and_then(|json| Ok((json, sidecar::report(host, &job, "html")?)));
            match html {
                Ok((json, html)) => match lhr::parse(&json) {
                    Ok(report) => Ok(Ok((report, json, html))),
                    Err(reason) => Ok(Err(format!("unreadable result: {reason}"))),
                },
                Err(failure) => Err(failure),
            }
        }
        Ok(Ended::Failed { message }) => Ok(Err(message)),
        Ok(Ended::Cancelled) => Err(Failure::new(FailureKind::Cancelled)),
        Err(failure) => Err(failure),
    };
    sidecar::release(host, &job);
    outcome
}

/// The `previous` part of a summary: the run id and scores of the summary in `latest.json`.
fn previous(host: &mut dyn PluginHost) -> Value {
    let Ok(bytes) = host.project_read(&out_path(LATEST)) else {
        return Value::Null;
    };
    let Ok(latest) = serde_json::from_slice::<Value>(&bytes) else {
        return Value::Null;
    };
    let breakpoints: Vec<Value> = latest["breakpoints"]
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .map(|entry| json!({ "breakpoint": entry["breakpoint"], "scores": entry["scores"] }))
        .collect();
    json!({ "runId": latest["runId"], "breakpoints": breakpoints })
}

fn write(host: &mut dyn PluginHost, path: &str, bytes: &[u8]) -> Result<(), Failure> {
    host.out_write(path, bytes)
}

/// Runs Lighthouse over the breakpoints `request` selects; answers the summary.
///
/// # Errors
///
/// No breakpoint to measure, no sidecar or browser, or the reports cannot be written.
pub fn run(host: &mut dyn PluginHost, request: &Request) -> Result<Value, Failure> {
    let project = Project::read(host);
    let views = host.views()?;
    let selection = Selection {
        only: request.breakpoints.as_deref(),
        view: request.view,
        url: request.url.as_deref(),
    };
    let (targets, skipped) = plan::targets(&views, &project, &selection);
    if targets.is_empty() {
        return Err(error(FailureKind::NotFound, "no_targets"));
    }
    let categories = plan::categories(request.categories.as_deref(), &project);
    let throttling = settings::throttling(host);
    let started_at = host.now_unix_seconds();
    let id = run_id(started_at);
    let previous = previous(host);
    let total = u32::try_from(targets.len()).unwrap_or(u32::MAX);
    let mut breakpoints = Vec::new();
    let mut failures = Vec::new();
    let mut findings: Vec<Finding> = Vec::new();
    let mut version = String::new();
    let mut cancelled = false;
    for (done, target) in (1..).zip(&targets) {
        match measure(host, target, throttling, &categories) {
            Ok(Ok((report, json, html))) => {
                let name = target.file_name();
                let (json_path, html_path) =
                    (format!("{id}/{name}.json"), format!("{id}/{name}.html"));
                write(host, &json_path, json.as_bytes())?;
                write(host, &html_path, html.as_bytes())?;
                let found = lhr::findings(&report, target.view, &target.url, &|_| true);
                version.clone_from(&report.lighthouse_version);
                breakpoints.push(json!({
                    "breakpoint": target.breakpoint, "view": target.view,
                    "width": target.width, "height": target.height, "dpr": target.dpr,
                    "formFactor": if target.mobile { "mobile" } else { "desktop" },
                    "url": target.url, "finalUrl": report.final_url,
                    "scores": report.scores, "runtimeError": report.runtime_error,
                    "report": out_path(&html_path), "json": out_path(&json_path),
                    "findings": found.len(),
                }));
                findings.extend(found);
            }
            Ok(Err(reason)) => failures.push(
                json!({ "breakpoint": target.breakpoint, "view": target.view, "reason": reason }),
            ),
            Err(failure) if failure.kind == FailureKind::Cancelled => {
                cancelled = true;
                break;
            }
            Err(failure) if breakpoints.is_empty() && failures.is_empty() => return Err(failure),
            Err(failure) => failures.push(json!({
                "breakpoint": target.breakpoint, "view": target.view,
                "reason": failure.code().unwrap_or("failed"),
            })),
        }
        host.report_progress(done, total);
    }
    let summary = json!({
        "version": SUMMARY_VERSION, "runId": id, "startedAt": started_at,
        "lighthouseVersion": version, "categories": categories, "throttling": throttling,
        "url": request.url, "breakpoints": breakpoints, "skipped": skipped_json(&skipped),
        "failures": failures, "cancelled": cancelled, "previous": previous,
        "findings": findings.len(),
    });
    let text = serde_json::to_vec_pretty(&summary).unwrap_or_default();
    write(host, &format!("{id}/run.json"), &text)?;
    write(host, LATEST, &text)?;
    if request.report
        && !findings.is_empty()
        && let Err(failure) = host.report_findings(&findings)
    {
        host.log("warn", &format!("findings not reported: {failure}"));
    }
    Ok(summary)
}

/// `check.run`: Lighthouse in the context's view (every breakpoint without one), its failed
/// audits of the rules switched on as findings. Nothing is saved; a Firefox or WebKit view has
/// no findings.
///
/// # Errors
///
/// No sidecar or browser, or the context's view's run failed.
pub fn check(host: &mut dyn PluginHost, ctx: &CheckContext) -> Result<Vec<Finding>, Failure> {
    let project = Project::read(host);
    let views = host.views()?;
    let selection = Selection {
        only: None,
        view: ctx.view,
        url: ctx.url.as_deref(),
    };
    let (targets, skipped) = plan::targets(&views, &project, &selection);
    // The one view a run names, of another engine: the runner shows it skipped, not "nothing
    // found" (RB-080-98).
    if let Some(view) = ctx
        .view
        .and_then(|id| views.iter().find(|view| view.id == id))
    {
        view.require_chromium("lighthouse")?;
    }
    for skip in &skipped {
        host.log(
            "info",
            &format!("{} skipped: {}", skip.breakpoint, skip.reason),
        );
    }
    let categories = plan::categories(None, &project);
    let throttling = settings::throttling(host);
    let enabled = |rule: &str| ctx.rules.iter().any(|on| on == rule);
    let mut findings = Vec::new();
    for target in &targets {
        match measure(host, target, throttling, &categories)? {
            Ok((report, _, _)) => {
                findings.extend(lhr::findings(&report, target.view, &target.url, &enabled))
            }
            Err(reason) => {
                return Err(
                    error(FailureKind::Other, "sidecar_failed").with_param("reason", reason)
                );
            }
        }
    }
    Ok(findings)
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
