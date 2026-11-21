//! The MCP tool `html_validate` and the command `plugin.html_validate.validate`: one run over
//! every view (or the view of `target`), its findings merged across the views - one entry per
//! rule and place in the source, naming the views and breakpoints it was found in. The command
//! also reports the findings into the findings panel (`findings.report`).
//!
//! Arguments: `{"target"?: "all" | <view id>, "engine"?: "builtin" | "nu", "source"?: "raw" |
//! "rendered"}` (`engine` and `source` override the settings for the call); the host resolves a
//! breakpoint id to its views and calls once per view. The answer: `{"engine", "source",
//! "projectRules": "none" | "untrusted" | "applied", "htmlValidate", "views": [{"id", "label",
//! "breakpoint", "engine", "findings" | "error"}], "findings": [...], "truncated"}` - a finding of
//! the rendered DOM has its `selector` and no `line`.

use std::collections::BTreeMap;

use rb_plugin_api::check::MAX_FINDINGS;
use rb_plugin_api::{Failure, FailureKind, Finding, PluginHost, Severity, ViewId, ViewInfo};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::settings::{Engine, Source};
use crate::{Cache, Plan, bad_args, rules, run_settings};

/// The most merged findings one answer lists; `truncated` beyond (as `checks_run`).
pub const MAX_ANSWER_FINDINGS: usize = 500;

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ToolArgs {
    target: Option<Value>,
    engine: Option<Engine>,
    source: Option<Source>,
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

/// One message at one place of one page, over the views.
struct Merged {
    finding: Finding,
    views: Vec<ViewId>,
    breakpoints: Vec<String>,
}

fn merge(found: Vec<(Finding, String)>) -> Vec<Merged> {
    let mut merged: BTreeMap<(String, String, String), Merged> = BTreeMap::new();
    for (finding, breakpoint) in found {
        let key = (
            finding.rule.clone(),
            finding.url.clone().unwrap_or_default(),
            finding.path.clone().unwrap_or_default(),
        );
        let view = finding.view.unwrap_or_default();
        let entry = merged.entry(key).or_insert_with(|| Merged {
            finding: finding.clone(),
            views: Vec::new(),
            breakpoints: Vec::new(),
        });
        if !entry.views.contains(&view) {
            entry.views.push(view);
        }
        if !breakpoint.is_empty() && !entry.breakpoints.contains(&breakpoint) {
            entry.breakpoints.push(breakpoint);
        }
    }
    let mut list: Vec<Merged> = merged.into_values().collect();
    list.sort_by_key(|merged| std::cmp::Reverse(merged.finding.severity));
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
        "severity": finding.severity.as_str(),
        "message": param(finding, "message"),
        "line": data["line"],
        "column": data["column"],
        "extract": data["extract"],
        "helpUrl": data["helpUrl"],
        "url": finding.url,
        "selector": finding.selector,
        "views": merged.views,
        "breakpoints": merged.breakpoints,
    })
}

/// Runs the validation over the views of `args`; `report` also hands the findings to the
/// findings panel.
///
/// # Errors
///
/// The arguments or settings do not read, the Nu engine has no address, the views cannot be
/// listed, every view failed, or the findings could not be reported.
pub fn validate(host: &mut dyn PluginHost, args: &str, report: bool) -> Result<String, Failure> {
    let args: ToolArgs = if args.trim().is_empty() {
        ToolArgs::default()
    } else {
        serde_json::from_str(args).map_err(|error| bad_args(&error.to_string()))?
    };
    let every: Vec<String> = rules::all().iter().map(|rule| rule.id.clone()).collect();
    let mut settings = run_settings(host)?;
    settings.source = args.source.unwrap_or(settings.source);
    let plan = Plan::new(settings, args.engine, every)?;
    let views = views_of(args.target.as_ref(), host.views()?)?;

    let total = u32::try_from(views.len()).unwrap_or(u32::MAX);
    let mut cache = Cache::new();
    let mut found = Vec::new();
    let mut checked = Vec::new();
    let mut first_failure = None;
    for (done, view) in (1..).zip(&views) {
        let mut status = json!({
            "id": view.id, "label": view.label, "breakpoint": view.breakpoint, "engine": view.engine,
        });
        match plan.run_view(host, view.id, &mut cache) {
            Ok(findings) => {
                status["findings"] = json!(findings.len());
                found.extend(findings.into_iter().map(|f| (f, view.breakpoint.clone())));
            }
            Err(failure) => {
                status["error"] = json!(failure.code().unwrap_or(failure.kind.as_str()));
                if let Some(reason) = failure.param("reason") {
                    status["reason"] = json!(reason);
                }
                first_failure.get_or_insert(failure);
            }
        }
        checked.push(status);
        host.report_progress(done, total);
    }
    if let Some(failure) = first_failure
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
    let errors = merged
        .iter()
        .filter(|m| m.finding.severity == Severity::Error)
        .count();
    Ok(json!({
        "engine": plan.engine.as_str(),
        "source": plan.settings.source.as_str(),
        "projectRules": plan.settings.project.state.as_str(),
        "htmlValidate": rules::html_validate_version(),
        "views": checked,
        "errors": errors,
        "findings": list,
        "truncated": truncated,
    })
    .to_string())
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
