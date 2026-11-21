//! The MCP tool `consent_check` (the panel and `checks_run` run the same), the command `check`
//! and the export: the run's capture - the core's probe session (`$consent`) when it supplies
//! one, the target view read only otherwise - judged, answered with the notice that this is a
//! technical pre-check and no legal advice and the caveats of the capture.

use rb_plugin_api::{Failure, FailureKind, Finding, PluginHost, Severity, ViewId, ViewInfo, codes};
use serde_json::{Map, Value, json};

use crate::analyze::{self, CookieRow, Diff, HostRow, PhaseReport, Report};
use crate::capture::{self, Mode, Run};
use crate::settings::Settings;
use crate::{NOTICE_KEY, NOTICE_TEXT, PANEL_ID, PLUGIN_ID, POST_KEY, cmp};

/// The export's file in the plugin's out folder.
pub const EXPORT_FILE: &str = "consent-check.json";
/// The longest `url` argument, in characters.
const MAX_URL_CHARS: usize = 2048;

/// The host's offer of a probe in a call's arguments (`true`), and the plugin's request in its
/// answer (RB-094-09).
pub const ASK_KEY: &str = "$consentProbe";

/// A check's answer and its findings; or, while `probing`, the request for the core's probe.
#[derive(Debug)]
pub struct Checked {
    pub answer: Value,
    pub findings: Vec<Finding>,
    /// The answer asks the core for a probe; the capture comes with the second call.
    pub probing: bool,
}

/// What a call works on: a run, or the probe it asks the core for.
enum Captured {
    Run(Run),
    Ask(Value),
}

fn web(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

/// The request for the core's probe of `url`: the collector in mode `read` and, with the reject
/// step on, in mode `reject`.
fn probe_request(url: &str, settings: &Settings) -> Value {
    let reject = settings
        .reject_step
        .then(|| cmp::collector(cmp::Mode::Reject));
    json!({ ASK_KEY: { "url": url, "read": cmp::collector(cmp::Mode::Read), "reject": reject } })
}

/// `plugins.error.invalid` with `reason`.
#[must_use]
pub fn invalid(reason: &str) -> Failure {
    Failure::coded(FailureKind::BadData, codes::INVALID).with_param("reason", reason)
}

/// The arguments as an object (`null` and none are `{}`).
///
/// # Errors
///
/// `plugins.error.invalid` for arguments that are no JSON object.
pub fn parse(args: &str) -> Result<Map<String, Value>, Failure> {
    if args.trim().is_empty() {
        return Ok(Map::new());
    }
    match serde_json::from_str::<Value>(args) {
        Ok(Value::Object(object)) => Ok(object),
        Ok(Value::Null) => Ok(Map::new()),
        _ => Err(invalid("args")),
    }
}

fn not_found(id: impl ToString) -> Failure {
    Failure::coded(FailureKind::NotFound, codes::NOT_FOUND).with_param("view", id.to_string())
}

/// The view `target` names, else `fallback`, else the active view (the first without one).
fn target(
    host: &mut dyn PluginHost,
    target: Option<&Value>,
    fallback: Option<ViewId>,
) -> Result<ViewInfo, Failure> {
    let views = host.views()?;
    let id = match target {
        None | Some(Value::Null) => fallback,
        Some(Value::String(all)) if all == "all" => fallback,
        Some(Value::Number(number)) => Some(
            number
                .as_u64()
                .and_then(|id| u32::try_from(id).ok())
                .ok_or_else(|| invalid("target"))?,
        ),
        Some(_) => return Err(invalid("target")),
    };
    let id = match id {
        Some(id) => Some(id),
        None => host.active_view()?,
    };
    match id {
        Some(id) => views
            .into_iter()
            .find(|view| view.id == id)
            .ok_or_else(|| not_found(id)),
        None => views.into_iter().next().ok_or_else(|| not_found("none")),
    }
}

fn without_fragment(url: &str) -> &str {
    url.split_once('#').map_or(url, |(base, _)| base)
}

/// `url` of the arguments: a string of at most [`MAX_URL_CHARS`].
fn url_arg(args: &Map<String, Value>) -> Result<Option<String>, Failure> {
    match args.get("url") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(url)) if !url.is_empty() && url.chars().count() <= MAX_URL_CHARS => {
            Ok(Some(url.clone()))
        }
        Some(_) => Err(invalid("url")),
    }
}

/// The capture of this call: the core's probe, else the view read.
///
/// With the host's offer (`$consentProbe`, the tool and the command) the answer asks for a
/// probe of `url` or the target view's page; the second call brings its capture (`$consent`).
/// Without the offer (`checks_run`) the target view is read as it is.
fn capture(
    host: &mut dyn PluginHost,
    args: &Map<String, Value>,
    view: Option<ViewId>,
    settings: &Settings,
) -> Result<Captured, Failure> {
    let url = url_arg(args)?;
    let whole = Value::Object(args.clone());
    if let Some(run) = capture::from_probe(&whole) {
        return Ok(Captured::Run(run));
    }
    if args.get(ASK_KEY) == Some(&Value::Bool(true)) {
        let page = match &url {
            Some(url) if web(url) => Some(url.clone()),
            Some(_) => return Err(invalid("url")),
            None => Some(target(host, args.get("target"), view)?.url).filter(|url| web(url)),
        };
        if let Some(page) = page {
            return Ok(Captured::Ask(probe_request(&page, settings)));
        }
    }
    let view = target(host, args.get("target"), view)?;
    // Another page needs the probe, which only the tool and the command are offered.
    if let Some(url) = url
        && without_fragment(&url) != without_fragment(&view.url)
    {
        return Err(
            Failure::coded(FailureKind::NotSupported, codes::NOT_AVAILABLE)
                .with_param("service", "a fresh consent session for another URL"),
        );
    }
    // Seconds since the epoch are far below 2^53: exact as f64.
    let now = host.now_unix_seconds() as f64;
    capture::from_view(host, &view, &whole, now).map(Captured::Run)
}

fn caveats(run: &Run, settings: &Settings) -> Vec<&'static str> {
    let mut list = Vec::new();
    match run.mode {
        Mode::View => {
            list.push("viewSession");
            list.push(if run.cookies_unread {
                "cookiesUnread"
            } else {
                "firstPartyCookies"
            });
            if settings.reject_step {
                list.push("noRejectStep");
            }
        }
        Mode::Probe => {
            if run.session_discarded != Some(true) {
                list.push("sessionKept");
            }
            if settings.reject_step && !run.banner.rejected {
                list.push("notRejected");
            }
        }
    }
    list
}

fn host_json(row: &HostRow) -> Value {
    json!({ "host": row.host, "kind": row.kind.as_str(), "count": row.count,
            "allowed": row.allowed, "example": row.example })
}

fn cookie_json(row: &CookieRow) -> Value {
    json!({ "name": row.name, "domain": row.domain, "days": row.days,
            "essential": row.essential, "thirdParty": row.third_party })
}

fn phase_json(report: &PhaseReport) -> Value {
    json!({
        "hosts": report.hosts.iter().map(host_json).collect::<Vec<_>>(),
        "firstParty": report.first_party,
        "cookies": report.cookies.iter().map(cookie_json).collect::<Vec<_>>(),
        "storage": report.storage,
    })
}

fn diff_json(diff: &Diff) -> Value {
    json!({
        "hostsAdded": diff.hosts_added, "hostsRemoved": diff.hosts_removed,
        "cookiesAdded": diff.cookies_added, "cookiesRemoved": diff.cookies_removed,
        "storageAdded": diff.storage_added, "storageRemoved": diff.storage_removed,
    })
}

fn answer(run: &Run, report: &Report, settings: &Settings) -> Value {
    let count = |severity: Severity| {
        report
            .findings
            .iter()
            .filter(|finding| finding.severity == severity)
            .count()
    };
    json!({
        "mode": run.mode.as_str(),
        "url": run.url,
        "site": report.site,
        "view": run.view,
        "notice": NOTICE_KEY,
        "noticeText": NOTICE_TEXT,
        "caveats": caveats(run, settings),
        "rejectStep": settings.reject_step,
        "banner": {
            "cmp": run.banner.cmp,
            "name": run.banner.cmp.as_deref().and_then(cmp::name_of),
            "visible": run.banner.visible,
            "rejected": run.banner.rejected,
        },
        "session": match run.mode {
            Mode::Probe => json!({ "fresh": true, "discarded": run.session_discarded }),
            Mode::View => json!({ "fresh": false, "discarded": null }),
        },
        "before": phase_json(&report.before),
        "afterReject": report.after.as_ref().map(phase_json),
        "diff": report.diff.as_ref().map(diff_json),
        "findings": report.findings,
        "summary": { "error": count(Severity::Error), "warning": count(Severity::Warning) },
    })
}

/// One check: the capture judged under the settings, the rules of `enabled` (all when empty),
/// the view `view` when the arguments name none; with `export: true` also written to
/// [`EXPORT_FILE`].
///
/// # Errors
///
/// Malformed arguments, a view that is gone, another URL without the probe session, the
/// collector's or the export's failure.
pub fn check(
    host: &mut dyn PluginHost,
    args: &Map<String, Value>,
    enabled: &[String],
    view: Option<ViewId>,
) -> Result<Checked, Failure> {
    let export = match args.get("export") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(export)) => *export,
        Some(_) => return Err(invalid("export")),
    };
    let settings = Settings::read(host);
    let run = match capture(host, args, view, &settings)? {
        Captured::Run(run) => run,
        Captured::Ask(request) => {
            return Ok(Checked {
                answer: request,
                findings: Vec::new(),
                probing: true,
            });
        }
    };
    let report = analyze::analyze(&run, &settings, enabled);
    let mut answer = answer(&run, &report, &settings);
    if export {
        let text = serde_json::to_string_pretty(&answer).map_err(|_| invalid("export"))?;
        host.out_write(EXPORT_FILE, text.as_bytes())?;
        answer["exported"] = json!(format!(".rbrowser/out/{PLUGIN_ID}/{EXPORT_FILE}"));
    }
    Ok(Checked {
        answer,
        findings: report.findings,
        probing: false,
    })
}

/// The command `check`: the check of the view `target` names (the active one by default), its
/// findings reported to the findings panel and the answer posted to the plugin's panel.
///
/// # Errors
///
/// As [`check`].
pub fn command(host: &mut dyn PluginHost, args: &Map<String, Value>) -> Result<Value, Failure> {
    let checked = check(host, args, &[], None)?;
    if checked.probing {
        return Ok(checked.answer);
    }
    let reported = host.report_findings(&checked.findings).is_ok();
    Ok(json!({
        "findings": checked.findings.len(),
        "reported": reported,
        POST_KEY: [{ "target": "panel",
                     "message": { "type": "checked", "panel": PANEL_ID, "answer": checked.answer } }],
    }))
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
