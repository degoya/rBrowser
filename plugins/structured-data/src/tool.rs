//! The MCP tool `structureddata_inspect` (the panel calls it too) and the command `check`.

use rb_plugin_api::{Failure, PluginHost, Severity};
use serde_json::{Map, Value, json};

use crate::rules::{self, Scope};
use crate::run::{self, Inspected, invalid};
use crate::settings::Settings;
use crate::validate::Issue;
use crate::{PANEL_ID, POST_KEY};

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

fn issue_json(issue: &Issue) -> Value {
    let params: Map<String, Value> = issue
        .params
        .iter()
        .map(|(key, value)| ((*key).to_owned(), Value::String(value.clone())))
        .collect();
    json!({
        "rule": issue.rule, "severity": rules::severity_of(issue.rule).as_str(),
        "message": crate::key(&format!("findings.{}", issue.rule)), "params": params,
        "node": issue.node, "property": issue.property, "place": issue.place,
    })
}

/// The answer of an inspection: the view, the page's blocks and nodes, the issues the settings
/// show with a count per severity; `views` lists every view for the panel's choice.
#[must_use]
pub fn answer(inspected: &Inspected, settings: &Settings, views: &[Value]) -> Value {
    let scope = Scope {
        enabled: &[],
        min: settings.min_severity,
        view: Some(inspected.view.id),
        url: &inspected.page.url,
    };
    let shown: Vec<&Issue> = inspected
        .issues
        .iter()
        .filter(|issue| scope.wants(issue.rule))
        .collect();
    let mut summary = json!({ "error": 0, "warning": 0, "info": 0 });
    for issue in &shown {
        let key = rules::severity_of(issue.rule).as_str();
        summary[key] = json!(summary[key].as_u64().unwrap_or(0) + 1);
    }
    json!({
        "view": { "id": inspected.view.id, "label": inspected.view.label, "url": inspected.view.url },
        "views": views,
        "url": inspected.page.url,
        "vocabulary": inspected.vocabulary,
        "richResults": settings.rich,
        "minSeverity": settings.min_severity.as_str(),
        "blocks": inspected.page.blocks,
        "items": inspected.page.nodes,
        "findings": shown.into_iter().map(issue_json).collect::<Vec<_>>(),
        "summary": summary,
        "truncated": inspected.page.truncated,
        "cut": false,
    })
}

fn size(value: &Value) -> usize {
    value.to_string().len()
}

/// `full` held to `budget` bytes: the raw texts of the blocks dropped first, then the top-level
/// items from the last; the findings stay. `cut` says so.
#[must_use]
pub fn fitted(mut full: Value, budget: Option<u64>) -> Value {
    let Some(budget) = budget else {
        return full;
    };
    let budget =
        usize::try_from(budget.clamp(BUDGET_RANGE.0, BUDGET_RANGE.1)).unwrap_or(usize::MAX);
    if size(&full) <= budget {
        return full;
    }
    full["cut"] = json!(true);
    if let Some(blocks) = full["blocks"].as_array_mut() {
        for block in blocks {
            block["text"] = Value::Null;
        }
    }
    if size(&full) <= budget {
        return full;
    }
    let items = match full["items"].take() {
        Value::Array(items) => items,
        _ => Vec::new(),
    };
    full["items"] = json!([]);
    let mut room = budget.saturating_sub(size(&full));
    let mut kept = Vec::new();
    for item in items {
        let cost = size(&item) + 1;
        if cost > room {
            break;
        }
        room -= cost;
        kept.push(item);
    }
    full["items"] = Value::Array(kept);
    full
}

fn view_list(host: &mut dyn PluginHost) -> Result<Vec<Value>, Failure> {
    Ok(host
        .views()?
        .into_iter()
        .map(|view| json!({ "id": view.id, "label": view.label, "url": view.url }))
        .collect())
}

/// MCP `structureddata_inspect`: the structured data of the page `target` shows (the active view
/// without one), with its issues; `budget` holds the answer to that many bytes of JSON.
///
/// # Errors
///
/// A bad `target` or `budget`, a view that is gone, or the collector's failure.
pub fn inspect(host: &mut dyn PluginHost, args: &Map<String, Value>) -> Result<Value, Failure> {
    let budget = match args.get("budget") {
        None | Some(Value::Null) => None,
        Some(value) => Some(value.as_u64().ok_or_else(|| invalid("budget"))?),
    };
    let view = run::target(host, args.get("target").unwrap_or(&Value::Null))?;
    let settings = Settings::read(host);
    let inspected = run::inspect(host, &view, &settings)?;
    let views = view_list(host)?;
    Ok(fitted(answer(&inspected, &settings, &views), budget))
}

/// The command `check`: the page of the view `target` names (the context menu's clicked view; the
/// active view without one), its findings into the findings panel; an open panel is told to load
/// it again.
///
/// # Errors
///
/// A bad `target`, no view or one that is gone, or the collector's failure.
pub fn check_command(
    host: &mut dyn PluginHost,
    args: &Map<String, Value>,
) -> Result<Value, Failure> {
    let view = run::target(host, args.get("target").unwrap_or(&Value::Null))?;
    let settings = Settings::read(host);
    let inspected = run::inspect(host, &view, &settings)?;
    let findings = inspected.findings(&settings, &[]);
    let reported = host.report_findings(&findings).is_ok();
    let errors = findings
        .iter()
        .filter(|finding| finding.severity == Severity::Error)
        .count();
    Ok(json!({
        "view": view.id, "findings": findings.len(), "errors": errors, "reported": reported,
        POST_KEY: [{ "target": "panel", "message": { "type": "checked", "panel": PANEL_ID } }],
    }))
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
