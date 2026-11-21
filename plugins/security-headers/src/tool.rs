//! The MCP tools `headers_check` and `headers_compare` (the panel calls them too) and the
//! command `check`.

use rb_plugin_api::{
    Failure, FailureKind, PluginHost, Severity, ViewInfo, codes, rules as host_rules,
};
use serde_json::{Map, Value, json};

use crate::compare::{self, Audited, COMPARED, Column};
use crate::cookies;
use crate::environments::{self, Environment};
use crate::levels::RuleLevels;
use crate::page::{self, Checked};
use crate::rules;
use crate::settings::Settings;
use crate::{OPEN_KEY, PANEL_ID, POST_KEY};

/// The most environments one comparison fetches.
pub const MAX_COMPARED: usize = 8;
/// The longest `url` taken.
const MAX_URL_BYTES: usize = 2048;

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

/// The view `target` names (a view id), else the active one, else the first; `None` without
/// views.
fn target_view(
    host: &mut dyn PluginHost,
    target: Option<&Value>,
) -> Result<Option<ViewInfo>, Failure> {
    let views = host.views()?;
    let id = match target {
        None | Some(Value::Null) => host.active_view()?,
        Some(Value::String(all)) if all == "all" => host.active_view()?,
        Some(value) => Some(
            value
                .as_u64()
                .and_then(|id| u32::try_from(id).ok())
                .ok_or_else(|| invalid("target"))?,
        ),
    };
    Ok(match id {
        Some(id) => Some(
            views
                .into_iter()
                .find(|view| view.id == id)
                .ok_or_else(|| {
                    Failure::coded(FailureKind::NotFound, codes::NOT_FOUND).with_param("view", id)
                })?,
        ),
        None => views.into_iter().next(),
    })
}

/// The URL a call checks: `url`, else the page of the view `target` names (the active view
/// without one).
///
/// # Errors
///
/// A `url` that is no `http(s)` URL, a bad `target`, no view, or a view showing no web page
/// (`errors.no_page`).
pub fn target_url(host: &mut dyn PluginHost, args: &Map<String, Value>) -> Result<String, Failure> {
    if let Some(url) = args.get("url").filter(|url| !url.is_null()) {
        let url = url.as_str().ok_or_else(|| invalid("url"))?;
        if url.len() > MAX_URL_BYTES || host_rules::check_http_url(url).is_err() {
            return Err(invalid("url"));
        }
        return Ok(url.to_owned());
    }
    let view = target_view(host, args.get("target"))?;
    view.map(|view| view.url)
        .filter(|url| host_rules::check_http_url(url).is_ok())
        .ok_or_else(|| crate::fetch::error(FailureKind::NotFound, "no_page"))
}

fn summary(findings: &[Value]) -> Value {
    let mut summary = json!({ "error": 0, "warning": 0, "info": 0 });
    for finding in findings {
        if let Some(severity) = finding["severity"].as_str() {
            summary[severity] = json!(summary[severity].as_u64().unwrap_or(0) + 1);
        }
    }
    summary
}

/// The answer of a check: the URL, its environment, the answer's status, the compared headers'
/// values, the cookies' attributes and the findings with a count per severity.
#[must_use]
pub fn answer(checked: &Checked, levels: &RuleLevels) -> Value {
    let findings: Vec<Value> = checked
        .findings(&[], levels)
        .iter()
        .map(rules::finding_json)
        .collect();
    let headers: Map<String, Value> = COMPARED
        .iter()
        .map(|name| {
            let value = checked
                .fetched
                .headers
                .get(name)
                .map(|value| compare::normalize(&value));
            ((*name).to_owned(), json!(value))
        })
        .collect();
    let cookies: Vec<Value> = checked
        .cookies
        .iter()
        .map(|cookie| json!({ "name": cookie.name, "attributes": cookies::summary(cookie) }))
        .collect();
    json!({
        "url": checked.url,
        "finalUrl": checked.fetched.final_url,
        "status": checked.fetched.status,
        "environment": checked.environment,
        "headers": headers,
        "cookies": cookies,
        "summary": summary(&findings),
        "findings": findings,
    })
}

fn checked(host: &mut dyn PluginHost, args: &Map<String, Value>) -> Result<Checked, Failure> {
    let url = target_url(host, args)?;
    let settings = Settings::read(host);
    let environments = environments::read(host);
    page::check_url(host, &url, &environments, &settings, true)
}

/// MCP `headers_check`: the page `url` (the active view's without one) checked.
///
/// # Errors
///
/// As [`target_url`] and [`page::check_url`].
pub fn check(host: &mut dyn PluginHost, args: &Map<String, Value>) -> Result<Value, Failure> {
    let checked = checked(host, args)?;
    let levels = RuleLevels::read(host);
    Ok(answer(&checked, &levels))
}

/// The command `check`: the page of the view `target` names (the active view without one)
/// checked, the findings into the findings panel.
///
/// # Errors
///
/// As [`check`].
pub fn check_command(
    host: &mut dyn PluginHost,
    args: &Map<String, Value>,
) -> Result<Value, Failure> {
    let checked = checked(host, args)?;
    let levels = RuleLevels::read(host);
    let findings = checked.findings(&[], &levels);
    let reported = host.report_findings(&findings).is_ok();
    let errors = findings
        .iter()
        .filter(|finding| finding.severity == Severity::Error)
        .count();
    Ok(json!({
        "url": checked.url, "environment": checked.environment,
        "findings": findings.len(), "errors": errors, "reported": reported,
    }))
}

/// The environments `args.environments` names (every project environment without it).
fn chosen(all: Vec<Environment>, args: &Map<String, Value>) -> Result<Vec<Environment>, Failure> {
    let chosen = match args.get("environments").filter(|list| !list.is_null()) {
        None => all,
        Some(list) => {
            let ids = list.as_array().ok_or_else(|| invalid("environments"))?;
            let mut chosen: Vec<Environment> = Vec::new();
            for id in ids.iter().take(MAX_COMPARED + 1) {
                let id = id.as_str().ok_or_else(|| invalid("environments"))?;
                let environment = all
                    .iter()
                    .find(|environment| environment.id == id)
                    .ok_or_else(|| {
                        crate::fetch::error(FailureKind::NotFound, "unknown_environment")
                            .with_param("environment", id.chars().take(64).collect::<String>())
                    })?;
                if !chosen.contains(environment) {
                    chosen.push(environment.clone());
                }
            }
            chosen
        }
    };
    if chosen.len() < 2 {
        return Err(crate::fetch::error(FailureKind::NotFound, "environments"));
    }
    if chosen.len() > MAX_COMPARED {
        return Err(invalid("environments"));
    }
    Ok(chosen)
}

/// MCP `headers_compare`: the page of the active view (its path, `/` without one) in each of
/// `environments` (every project environment without it), compared.
///
/// # Errors
///
/// Fewer than two environments, an unknown one, more than [`MAX_COMPARED`]; a fetch that fails
/// is shown in its column, no error.
pub fn compare(host: &mut dyn PluginHost, args: &Map<String, Value>) -> Result<Value, Failure> {
    let all = environments::read(host);
    let chosen = chosen(all.clone(), args)?;
    let path = target_view(host, None)
        .ok()
        .flatten()
        .map_or_else(|| "/".to_owned(), |view| environments::path_of(&view.url));
    let settings = Settings::read(host);
    let levels = RuleLevels::read(host);
    let total = u32::try_from(chosen.len()).unwrap_or(u32::MAX);
    let mut columns = Vec::new();
    for (done, environment) in chosen.iter().enumerate() {
        let url = environments::rebase(environment, &path);
        let outcome = page::check_url(host, &url, &all, &settings, false).map(|checked| Audited {
            final_url: checked.fetched.final_url.clone(),
            status: checked.fetched.status,
            findings: checked.findings(&[], &levels),
            headers: checked.fetched.headers,
            cookies: checked.cookies,
        });
        columns.push(Column {
            id: environment.id.clone(),
            url,
            outcome,
        });
        host.report_progress(u32::try_from(done + 1).unwrap_or(u32::MAX), total);
    }
    Ok(compare::table(&path, &columns))
}

/// The command `compare_environments`: the panel shown at the comparison, which it runs.
#[must_use]
pub fn compare_command() -> Value {
    json!({
        OPEN_KEY: { "panel": PANEL_ID },
        POST_KEY: [{ "target": "panel", "message": { "type": "compare" } }],
    })
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
