//! The MCP tool `a11y_check` and the command `plugin.a11y_axe.check`: one run over every view
//! (or the view of `target`), its findings merged across the views - one entry per rule and
//! element, naming the views and breakpoints it was found in. The command also reports the
//! findings into the findings panel (`findings.report`).
//!
//! Arguments: `{"target"?: "all" | <view id>, "tags"?: [<axe tag>]}`; the host resolves a
//! breakpoint id to its views and calls once per view. The answer:
//! `{"axeVersion", "tags", "rules", "views": [{"id", "label", "breakpoint", "engine", "findings"
//! | "error"}], "findings": [...], "truncated"}`.

use std::collections::BTreeMap;

use rb_plugin_api::check::MAX_FINDINGS;
use rb_plugin_api::{Failure, FailureKind, Finding, PluginHost, Severity, ViewId, ViewInfo};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::settings::{self, Settings};
use crate::{Plan, bad_args, rules};

/// The most merged findings one answer lists; `truncated` beyond (as `checks_run`).
pub const MAX_ANSWER_FINDINGS: usize = 500;

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ToolArgs {
    target: Option<Value>,
    tags: Option<Vec<String>>,
}

/// The views of `target`: every view for none or `"all"`, the one named by an id.
fn views_of(target: Option<&Value>, views: Vec<ViewInfo>) -> Result<Vec<ViewInfo>, Failure> {
    match target {
        None => Ok(views),
        Some(Value::String(all)) if all == "all" => Ok(views),
        Some(Value::Number(number)) => {
            let id = number
                .as_u64()
                .and_then(|id| ViewId::try_from(id).ok())
                .ok_or_else(|| bad_args("target: not a view id"))?;
            let found: Vec<ViewInfo> = views.into_iter().filter(|view| view.id == id).collect();
            Ok(if found.is_empty() {
                vec![ViewInfo {
                    id,
                    ..ViewInfo::default()
                }]
            } else {
                found
            })
        }
        Some(_) => Err(bad_args("target: \"all\" or a view id")),
    }
}

/// The URL without its fragment, as the findings' ids see it.
fn page(url: Option<&str>) -> String {
    let url = url.unwrap_or_default().trim();
    url.split_once('#').map_or(url, |(page, _)| page).to_owned()
}

/// One rule at one element of one page, over the views.
struct Merged {
    finding: Finding,
    severity: Severity,
    views: Vec<ViewId>,
    breakpoints: Vec<String>,
}

fn merge(found: Vec<(Finding, String)>) -> Vec<Merged> {
    let mut merged: BTreeMap<(String, String, String), Merged> = BTreeMap::new();
    for (finding, breakpoint) in found {
        let element = finding
            .path
            .clone()
            .or_else(|| finding.selector.clone())
            .unwrap_or_default();
        let key = (finding.rule.clone(), page(finding.url.as_deref()), element);
        let view = finding.view.unwrap_or_default();
        let entry = merged.entry(key).or_insert_with(|| Merged {
            severity: finding.severity,
            finding: finding.clone(),
            views: Vec::new(),
            breakpoints: Vec::new(),
        });
        entry.severity = entry.severity.max(finding.severity);
        if !entry.views.contains(&view) {
            entry.views.push(view);
        }
        if !breakpoint.is_empty() && !entry.breakpoints.contains(&breakpoint) {
            entry.breakpoints.push(breakpoint);
        }
    }
    let mut list: Vec<Merged> = merged.into_values().collect();
    list.sort_by(|a, b| b.severity.cmp(&a.severity));
    list
}

fn param<'a>(finding: &'a Finding, name: &str) -> &'a str {
    finding
        .params
        .iter()
        .find(|(key, _)| key == name)
        .map_or("", |(_, value)| value.as_str())
}

fn entry(merged: &Merged) -> Value {
    let finding = &merged.finding;
    let data: Value = finding
        .data
        .as_deref()
        .and_then(|data| serde_json::from_str(data).ok())
        .unwrap_or(Value::Null);
    json!({
        "rule": finding.rule,
        "sharedId": rules::shared_id(&finding.rule),
        "severity": merged.severity.as_str(),
        "help": param(finding, "help"),
        "impact": param(finding, "impact"),
        "needsReview": data["needsReview"],
        "helpUrl": data["helpUrl"],
        "wcag": data["wcag"],
        "html": data["html"],
        "url": finding.url,
        "selector": finding.selector,
        "path": finding.path,
        "views": merged.views,
        "breakpoints": merged.breakpoints,
    })
}

/// Runs the check over the views of `args`; `report` also hands the findings to the findings
/// panel.
///
/// # Errors
///
/// The arguments or settings do not read, the views cannot be listed, every view failed, or the
/// findings could not be reported.
pub fn check(host: &mut dyn PluginHost, args: &str, report: bool) -> Result<String, Failure> {
    let args: ToolArgs = if args.trim().is_empty() {
        ToolArgs::default()
    } else {
        serde_json::from_str(args).map_err(|error| bad_args(&error.to_string()))?
    };
    if let Some(tags) = &args.tags {
        settings::check_tags("tags", tags).map_err(|_| bad_args("tags: not a list of axe tags"))?;
    }
    let settings = Settings::read(host)?;
    let every: Vec<String> = rules::all().iter().map(|rule| rule.id.clone()).collect();
    let plan = Plan::new(&settings, args.tags.as_deref(), &every);
    let views = views_of(args.target.as_ref(), host.views()?)?;

    let total = u32::try_from(views.len()).unwrap_or(u32::MAX);
    let mut found = Vec::new();
    let mut checked = Vec::new();
    let mut first_failure = None;
    for (done, view) in (1..).zip(&views) {
        let mut status = json!({
            "id": view.id, "label": view.label, "breakpoint": view.breakpoint, "engine": view.engine,
        });
        match plan.run_view(host, view.id) {
            Ok((_, findings)) => {
                status["findings"] = json!(findings.len());
                found.extend(findings.into_iter().map(|f| (f, view.breakpoint.clone())));
            }
            Err(failure) => {
                status["error"] = json!(failure.code().unwrap_or(failure.kind.as_str()));
                first_failure.get_or_insert(failure);
            }
        }
        checked.push(status);
        host.report_progress(done, total);
    }
    if let Some(failure) = first_failure
        && found.is_empty()
        && checked.iter().all(|status| status.get("error").is_some())
    {
        return Err(failure);
    }
    if report {
        // The host refuses a longer list as a whole (`findings`: at most 10 000).
        let findings: Vec<Finding> = found
            .iter()
            .take(MAX_FINDINGS)
            .map(|(finding, _)| finding.clone())
            .collect();
        match host.report_findings(&findings) {
            Err(failure) if failure.kind == FailureKind::NotSupported => {
                host.log("warn", &format!("findings not reported: {failure}"));
            }
            other => other?,
        }
    }
    let merged = merge(found);
    let truncated = merged.len() > MAX_ANSWER_FINDINGS;
    let list: Vec<Value> = merged.iter().take(MAX_ANSWER_FINDINGS).map(entry).collect();
    let tags = args.tags.unwrap_or(settings.tags);
    Ok(json!({
        "axeVersion": rules::axe_version(),
        "tags": tags,
        "rules": plan.rules().len(),
        "views": checked,
        "findings": list,
        "truncated": truncated,
    })
    .to_string())
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
