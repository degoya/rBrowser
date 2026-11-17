//! The MCP tools `pwa_check` and `pwa_icon` (the panel calls them too) and the command `check`.

use rb_plugin_api::{Failure, FailureKind, Finding, PluginHost, Severity, codes};
use serde_json::{Map, Value, json};

use crate::base64;
use crate::fetch::{self, ACCEPT_IMAGE, Budget, Fetched};
use crate::image::sniff;
use crate::run::{self, Run, invalid};
use crate::settings::Settings;
use crate::url::Url;
use crate::{PANEL_ID, POST_KEY};

/// The out file of the last check (`.rbrowser/out/pwa-check/report.json`).
pub const REPORT_FILE: &str = "report.json";
/// The largest icon answered as a `data:` URI: its base64 stays below the panel's 64 KiB.
pub const MAX_ICON_BYTES: usize = 46 * 1024;
/// The smallest and largest `budget` of an answer, in bytes of JSON.
const BUDGET_RANGE: (u64, u64) = (4096, 4 * 1024 * 1024);

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

fn severity(severity: Severity) -> &'static str {
    match severity {
        Severity::Info => "info",
        Severity::Warning => "warning",
        Severity::Error => "error",
    }
}

fn finding_json(finding: &Finding, with_data: bool) -> Value {
    let params: Map<String, Value> = finding
        .params
        .iter()
        .map(|(key, value)| (key.clone(), Value::String(value.clone())))
        .collect();
    let mut value = json!({
        "rule": finding.rule, "severity": severity(finding.severity), "message": finding.message,
        "params": params, "view": finding.view, "url": finding.url, "path": finding.path,
    });
    if with_data && let Some(data) = finding.data.as_deref() {
        value["data"] = serde_json::from_str(data).unwrap_or(Value::Null);
    }
    value
}

/// The answer of a run: its pages, findings, a count per severity and the skipped views.
#[must_use]
pub fn answer(run: &Run, with_data: bool, pages: usize) -> Value {
    let mut summary = json!({ "error": 0, "warning": 0, "info": 0 });
    for finding in &run.findings {
        let key = severity(finding.severity);
        summary[key] = json!(summary[key].as_u64().unwrap_or(0) + 1);
    }
    let shown: Vec<_> = run.pages.iter().take(pages).collect();
    let findings: Vec<Value> = run
        .findings
        .iter()
        .filter(|finding| shown.iter().any(|page| Some(page.view) == finding.view))
        .map(|finding| finding_json(finding, with_data))
        .collect();
    json!({
        "pages": shown, "findings": findings, "summary": summary, "skipped": run.skipped,
        "cut": pages < run.pages.len() || !with_data,
    })
}

/// [`answer`] held to `budget` bytes: the findings' panel places dropped first, then the pages
/// after the first.
#[must_use]
pub fn fitted(run: &Run, budget: Option<u64>) -> Value {
    let full = answer(run, true, run.pages.len());
    let Some(budget) = budget else {
        return full;
    };
    let budget =
        usize::try_from(budget.clamp(BUDGET_RANGE.0, BUDGET_RANGE.1)).unwrap_or(usize::MAX);
    if full.to_string().len() <= budget {
        return full;
    }
    let lean = answer(run, false, run.pages.len());
    if lean.to_string().len() <= budget {
        return lean;
    }
    answer(run, false, 1)
}

/// MCP `pwa_check`: the views of `target` (every view without one), each page once; the report
/// also goes to `.rbrowser/out/pwa-check/report.json` (best effort).
///
/// # Errors
///
/// A bad target or `budget`, or a view that is gone.
pub fn check(host: &mut dyn PluginHost, args: &Map<String, Value>) -> Result<Value, Failure> {
    let budget = match args.get("budget") {
        None | Some(Value::Null) => None,
        Some(value) => Some(value.as_u64().ok_or_else(|| invalid("budget"))?),
    };
    let views = run::targets(host, args.get("target").unwrap_or(&Value::Null))?;
    let settings = Settings::read(host);
    let run = run::run(host, &settings, &views, &[]);
    let full = answer(&run, true, run.pages.len());
    let written = host
        .out_write(REPORT_FILE, full.to_string().as_bytes())
        .is_ok();
    let mut value = fitted(&run, budget);
    value["written"] = json!(written);
    Ok(value)
}

/// The command `check`: every view's page, the findings into the findings panel (in every view
/// showing the page); an open panel is told to load the new report.
///
/// # Errors
///
/// The views cannot be listed.
pub fn check_command(host: &mut dyn PluginHost) -> Result<Value, Failure> {
    let settings = Settings::read(host);
    let views = host.views()?;
    let run = run::run(host, &settings, &views, &[]);
    let full = answer(&run, true, run.pages.len());
    let _ = host.out_write(REPORT_FILE, full.to_string().as_bytes());
    let findings = run.findings_per_view();
    let reported = host.report_findings(&findings).is_ok();
    Ok(json!({
        "pages": run.pages.len(), "findings": run.findings.len(), "reported": reported,
        POST_KEY: [{ "target": "panel", "message": { "type": "checked", "panel": PANEL_ID } }],
    }))
}

/// MCP `pwa_icon`: the icon at `url`, which must lie on the origin of a page a Chromium view
/// shows, with its real format and size, and as a `data:` URI up to [`MAX_ICON_BYTES`].
///
/// # Errors
///
/// `plugins.error.invalid` (`url`, `origin`, `not_image`), `plugins.error.not_found` (an answer
/// other than 200, with its `status`), or the host's failure.
pub fn icon(host: &mut dyn PluginHost, args: &Map<String, Value>) -> Result<Value, Failure> {
    let url = args
        .get("url")
        .and_then(Value::as_str)
        .and_then(Url::parse)
        .ok_or_else(|| invalid("url"))?;
    let page = host
        .views()?
        .into_iter()
        .filter(|view| view.engine == "chromium")
        .filter_map(|view| Url::parse(&view.url))
        .find(|page| page.same_origin(&url))
        .ok_or_else(|| invalid("origin"))?;
    let settings = Settings::read(host);
    let budget = Budget::new(host, settings.timeout_ms);
    let response = match fetch::get(host, &budget, &page, &url, ACCEPT_IMAGE) {
        Fetched::Ok(response) => response,
        Fetched::OtherOrigin(_) => return Err(invalid("origin")),
        Fetched::Failed(code) => {
            return Err(
                Failure::coded(FailureKind::Io, codes::HTTP_FAILED).with_param("reason", code)
            );
        }
    };
    if response.status != 200 {
        return Err(Failure::coded(FailureKind::NotFound, codes::NOT_FOUND)
            .with_param("status", response.status));
    }
    let image = sniff(&response.body).ok_or_else(|| invalid("not_image"))?;
    let bytes = response.body.len();
    let data_uri = (bytes <= MAX_ICON_BYTES).then(|| {
        format!(
            "data:{};base64,{}",
            image.format.mime(),
            base64::encode(&response.body)
        )
    });
    Ok(json!({
        "url": url.to_string(), "format": image.format.mime(),
        "width": image.size.map(|(w, _)| w), "height": image.size.map(|(_, h)| h),
        "bytes": bytes, "dataUri": data_uri, "tooLarge": data_uri.is_none(),
    }))
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
