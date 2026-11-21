//! The endpoint analysis' tools (RB-096-06): `endpoints` (MCP `networkpanel_endpoints`, the tab
//! Endpoints), `endpoints_check` (the command and MCP `networkpanel_endpoints_check`: the
//! findings into the findings panel) and the check run of `checks_run`. All of it reads the
//! host's capture of the views' requests - nothing is sent. An endpoint carries the rules it
//! breaks on the current page (`issues`); every URL is masked.

use std::collections::{BTreeMap, BTreeSet};

use rb_plugin_api::{Failure, Finding, PluginHost, ViewId};
use serde_json::{Map, Value, json};

use crate::calls::calls;
use crate::context::{Context, FEED_KEY};
use crate::endpoint_rules::{self, Page};
use crate::endpoint_settings::{EndpointSettings, known_types};
use crate::endpoint_summary::summary;
use crate::endpoints::{Endpoint, Options, group};
use crate::levels::RuleLevels;
use crate::{OPEN_KEY, PANEL_ID, bad_args};

/// The endpoints of an answer by default and at most.
pub const LIMIT: (usize, usize) = (100, 500);
/// The JSON bytes of the endpoints of one answer, and of its findings: a panel takes 64 KiB.
pub const BUDGET: (usize, usize) = (40 * 1024, 12 * 1024);
/// The orders an answer sorts by, the first the default: the slowest first.
pub const SORTS: &[&str] = &["p95", "calls", "size", "ttfb"];

fn pages(host: &mut dyn PluginHost) -> Vec<Page> {
    host.views()
        .unwrap_or_default()
        .into_iter()
        .map(|view| Page {
            view: view.id,
            label: view.label,
            url: view.url,
        })
        .collect()
}

/// MCP's `target` as one view (`"all"` or none: every view).
fn target(args: &Value) -> Option<ViewId> {
    args["target"]
        .as_u64()
        .and_then(|view| ViewId::try_from(view).ok())
}

/// The findings of the current page under the person's levels.
fn found(
    host: &mut dyn PluginHost,
    endpoints: &[Endpoint<'_>],
    settings: &EndpointSettings,
    rules: &[String],
    view: Option<ViewId>,
) -> Vec<Finding> {
    let pages = pages(host);
    let levels = RuleLevels::read(host);
    endpoint_rules::findings(endpoints, &pages, &settings.limits, rules, view)
        .into_iter()
        .filter_map(|finding| levels.apply(finding))
        .collect()
}

/// The check run's findings: the rules `rules` (empty: all) over the capture in `args`, only
/// `view`'s when the run names one.
#[must_use]
pub fn check(
    host: &mut dyn PluginHost,
    args: &Value,
    rules: &[String],
    view: Option<ViewId>,
) -> Vec<Finding> {
    let settings = EndpointSettings::read(host);
    let all = calls(args);
    let options = Options {
        types: settings.types.clone(),
        templates: settings.templates.clone(),
        current_only: true,
        ..Options::default()
    };
    let endpoints = group(&all, &options);
    found(host, &endpoints, &settings, rules, view)
}

fn options(args: &Value, settings: &EndpointSettings) -> Result<Options, Failure> {
    let types = match &args["types"] {
        Value::Null => settings.types.clone(),
        Value::Array(list) => {
            let known = known_types(list);
            if known.len() != list.len() {
                return Err(bad_args("types"));
            }
            known
        }
        _ => return Err(bad_args("types")),
    };
    let search = match &args["search"] {
        Value::Null => None,
        Value::String(text) if text.trim().is_empty() => None,
        Value::String(text) => Some(
            text.trim()
                .chars()
                .take(200)
                .collect::<String>()
                .to_lowercase(),
        ),
        _ => return Err(bad_args("search")),
    };
    let current_only = match &args["currentPage"] {
        Value::Null => false,
        Value::Bool(on) => *on,
        _ => return Err(bad_args("currentPage")),
    };
    Ok(Options {
        types,
        templates: settings.templates.clone(),
        view: target(args),
        current_only,
        search,
    })
}

fn sort_key(endpoint: &Value, sort: &str) -> f64 {
    let number = match sort {
        "calls" => endpoint["calls"].as_f64(),
        "size" => endpoint["decoded"]["total"].as_f64(),
        "ttfb" => endpoint["ttfbMs"]["p95"].as_f64(),
        _ => endpoint["durationMs"]["p95"].as_f64(),
    };
    number.unwrap_or(-1.0)
}

/// A finding as an answer lists it, with the key of the endpoint it is about.
fn finding_json(finding: &Finding) -> Value {
    let params: Map<String, Value> = finding
        .params
        .iter()
        .map(|(key, value)| (key.clone(), Value::String(value.clone())))
        .collect();
    json!({ "rule": finding.rule, "severity": finding.severity.as_str(), "message": finding.message,
            "params": params, "view": finding.view, "path": finding.path,
            "endpoint": finding.data.as_deref().and_then(endpoint_of) })
}

/// Values of `items` while their JSON stays within `budget` bytes.
fn within(items: impl Iterator<Item = Value>, budget: usize) -> Vec<Value> {
    let mut used = 0;
    let mut out = Vec::new();
    for item in items {
        used += item.to_string().len() + 1;
        if used > budget {
            break;
        }
        out.push(item);
    }
    out
}

/// `endpoints`: the endpoints of the capture (`types`, `search`, `currentPage`, MCP's `target`),
/// sorted by `sort` (`p95` by default), at most `limit`; the views and the current page's
/// findings.
///
/// # Errors
///
/// Arguments that do not read.
pub fn endpoints(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let settings = EndpointSettings::read(host);
    let options = options(args, &settings)?;
    let sort = match &args["sort"] {
        Value::Null => SORTS[0],
        Value::String(sort) if SORTS.contains(&sort.as_str()) => sort.as_str(),
        _ => return Err(bad_args("sort")),
    };
    let limit = match &args["limit"] {
        Value::Null => LIMIT.0,
        value => value
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| (1..=LIMIT.1).contains(n))
            .ok_or_else(|| bad_args("limit"))?,
    };
    let all = calls(args);
    let grouped = group(&all, &options);
    let current = group(
        &all,
        &Options {
            current_only: true,
            ..options.clone()
        },
    );
    let findings = found(host, &current, &settings, &[], options.view);
    let mut issues: BTreeMap<String, BTreeSet<&str>> = BTreeMap::new();
    for finding in &findings {
        if let Some(key) = finding.data.as_deref().and_then(endpoint_of) {
            issues.entry(key).or_default().insert(finding.rule.as_str());
        }
    }
    let mut summaries: Vec<Value> = grouped
        .iter()
        .map(|endpoint| {
            let mut value = summary(endpoint);
            value["issues"] = json!(
                issues
                    .get(endpoint.key.as_str())
                    .cloned()
                    .unwrap_or_default()
            );
            value
        })
        .collect();
    summaries.sort_by(|a, b| {
        sort_key(b, sort)
            .total_cmp(&sort_key(a, sort))
            .then_with(|| a["key"].as_str().cmp(&b["key"].as_str()))
    });
    let total = summaries.len();
    let context = Context::read(args);
    Ok(json!({
        "endpoints": within(summaries.into_iter().take(limit), BUDGET.0),
        "total": total,
        "captured": args[FEED_KEY].is_object(),
        "views": crate::tools::views(host, &context),
        "findings": within(findings.iter().map(finding_json), BUDGET.1),
        "sort": sort,
    }))
}

/// The endpoint key a finding's details (this plugin's own JSON) name.
fn endpoint_of(data: &str) -> Option<String> {
    let details: Value = serde_json::from_str(data).ok()?;
    details["endpoint"].as_str().map(str::to_owned)
}

/// `endpoints_check`: the current page's findings of every rule into the findings panel.
///
/// # Errors
///
/// None so far; a refused report is said in the answer (`reported: false`).
pub fn endpoints_check(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let findings = check(host, args, &[], target(args));
    let reported = host.report_findings(&findings).is_ok();
    Ok(json!({ "findings": findings.len(), "reported": reported }))
}

/// The command `endpoints`: the network panel on its tab Endpoints.
#[must_use]
pub fn open() -> Value {
    json!({
        OPEN_KEY: { "panel": PANEL_ID },
        "$post": [{ "target": "panel", "message": { "tab": "endpoints" } }],
    })
}

#[cfg(test)]
#[path = "endpoint_tool_tests.rs"]
mod tests;
