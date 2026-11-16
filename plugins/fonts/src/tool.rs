//! The MCP tools and the panel's calls: `list` (the inventory over the views, with the findings),
//! `check` (the findings), `mark` (where a family is used, in every view), `export` (JSON and CSV
//! into `.rbrowser/out/fonts/`), and the command `check` (the findings into the findings panel).

use rb_plugin_api::{Failure, FailureKind, Finding, PluginHost, codes};
use serde_json::{Value, json};

use crate::checks;
use crate::export;
use crate::fluid;
use crate::inventory;
use crate::page;
use crate::run::{self, Collected, ViewFonts};
use crate::settings::Settings;

/// The smallest and largest answer budget a caller may set, in bytes of JSON.
pub const MIN_BUDGET: usize = 4 * 1024;
pub const MAX_BUDGET: usize = 4 * 1024 * 1024;
/// The longest family a mark takes, in characters.
pub const MAX_FAMILY_CHARS: usize = 200;

fn invalid(reason: &str) -> Failure {
    Failure::coded(FailureKind::BadData, codes::INVALID).with_param("reason", reason)
}

/// The arguments as JSON (`{}` for none).
///
/// # Errors
///
/// `plugins.error.invalid` when they are no JSON object.
pub fn parse(args: &str) -> Result<Value, Failure> {
    if args.trim().is_empty() {
        return Ok(json!({}));
    }
    let value: Value = serde_json::from_str(args).map_err(|_| invalid("arguments"))?;
    if value.is_object() {
        Ok(value)
    } else {
        Err(invalid("arguments"))
    }
}

fn flag(args: &Value, name: &str, default: bool) -> Result<bool, Failure> {
    match args.get(name) {
        None | Some(Value::Null) => Ok(default),
        Some(value) => value.as_bool().ok_or_else(|| invalid(name)),
    }
}

/// The JSON of a finding, as `fonts_check` and `fonts_list` answer it.
#[must_use]
pub fn finding_json(finding: &Finding) -> Value {
    let params: serde_json::Map<String, Value> = finding
        .params
        .iter()
        .map(|(key, value)| (key.clone(), Value::String(value.clone())))
        .collect();
    json!({
        "rule": finding.rule,
        "severity": finding.severity.as_str(),
        "message": finding.message,
        "params": params,
        "view": finding.view,
        "url": finding.url,
        "selector": finding.selector,
        "path": finding.path,
    })
}

/// The findings of `collected`: each view's, and the breakpoint rule over the views of each page.
#[must_use]
pub fn findings(collected: &Collected, settings: &Settings) -> Vec<Finding> {
    let mut list: Vec<Finding> = collected
        .views
        .iter()
        .flat_map(|view| checks::view(&[], view, settings))
        .collect();
    let mut urls: Vec<&str> = Vec::new();
    for view in &collected.views {
        if urls.contains(&view.view.url.as_str()) {
            continue;
        }
        urls.push(&view.view.url);
        let group: Vec<&ViewFonts> = collected
            .views
            .iter()
            .filter(|other| other.view.url == view.view.url)
            .collect();
        list.extend(fluid::findings(&[], view.view.id, &group, settings));
    }
    list
}

fn collect(
    host: &mut dyn PluginHost,
    args: &Value,
    settings: &Settings,
) -> Result<Collected, Failure> {
    let rendered = flag(args, "rendered", true)?;
    let views = run::targets(host, &args["target"])?;
    Ok(run::collect(host, &views, settings, rendered))
}

/// The inventory of `collected` as `list` and the export answer it.
#[must_use]
pub fn inventory_json(collected: &Collected, settings: &Settings) -> Value {
    let group: Vec<&ViewFonts> = collected.views.iter().collect();
    let mut sheets: Vec<&Value> = Vec::new();
    for sheet in collected.views.iter().flat_map(|view| &view.fonts.sheets) {
        if !sheets.contains(&sheet) {
            sheets.push(sheet);
        }
    }
    json!({
        "views": inventory::views(collected),
        "families": inventory::families(collected),
        "roles": fluid::json(&group, settings),
        "files": inventory::files(collected),
        "sheets": sheets,
    })
}

/// Cuts the longest lists of `answer` until its JSON fits `budget` bytes; marks it `cut`.
pub fn fit(answer: &mut Value, budget: usize) {
    const LISTS: [&str; 4] = ["findings", "files", "roles", "families"];
    while answer.to_string().len() > budget {
        let longest = LISTS
            .iter()
            .filter_map(|name| answer[*name].as_array().map(|list| (*name, list.len())))
            .filter(|(_, len)| *len > 0)
            .max_by_key(|(_, len)| *len);
        let Some((name, len)) = longest else {
            break;
        };
        if let Some(list) = answer[name].as_array_mut() {
            list.truncate(len - len.div_ceil(5));
        }
        answer["cut"] = Value::Bool(true);
    }
}

fn budget(args: &Value) -> Result<Option<usize>, Failure> {
    match args.get("budget") {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| (MIN_BUDGET..=MAX_BUDGET).contains(n))
            .map(Some)
            .ok_or_else(|| invalid("budget")),
    }
}

/// `list`: the inventory of the target's views (`rendered: false` skips the CDP part), with
/// the findings; `budget` cuts the lists to fit.
///
/// # Errors
///
/// `plugins.error.invalid` for malformed arguments, the target's failure.
pub fn list(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let budget = budget(args)?;
    let settings = Settings::read(host);
    let collected = collect(host, args, &settings)?;
    let mut answer = inventory_json(&collected, &settings);
    let found: Vec<Value> = findings(&collected, &settings)
        .iter()
        .map(finding_json)
        .collect();
    answer["findings"] = Value::Array(found);
    if let Some(budget) = budget {
        fit(&mut answer, budget);
    }
    Ok(answer)
}

/// `check`: the findings of the target's views, with a count per rule.
///
/// # Errors
///
/// `plugins.error.invalid` for malformed arguments, the target's failure.
pub fn check(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let settings = Settings::read(host);
    let collected = collect(host, args, &settings)?;
    let found = findings(&collected, &settings);
    let mut summary = serde_json::Map::new();
    for finding in &found {
        let count = summary.entry(finding.rule.clone()).or_insert(json!(0));
        *count = json!(count.as_u64().unwrap_or(0) + 1);
    }
    Ok(json!({
        "views": inventory::views(&collected),
        "summary": summary,
        "findings": found.iter().map(finding_json).collect::<Vec<_>>(),
    }))
}

/// `mark`: the text set in `family` (and `weight`, `style`) outlined in the target's views, or
/// `clear` (`true`) every mark.
///
/// # Errors
///
/// `plugins.error.invalid` for malformed arguments, the target's failure.
pub fn mark(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let views = run::targets(host, &args["target"])?;
    if flag(args, "clear", false)? {
        for view in &views {
            let _ = page::clear(host, view.id);
        }
        return Ok(json!({ "cleared": true }));
    }
    let family = args["family"]
        .as_str()
        .map(str::trim)
        .filter(|f| !f.is_empty() && f.chars().count() <= MAX_FAMILY_CHARS)
        .filter(|f| !f.chars().any(char::is_control))
        .ok_or_else(|| invalid("family"))?;
    let weight = match args.get("weight") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_u64()
                .filter(|w| (1..=1000).contains(w))
                .ok_or_else(|| invalid("weight"))?,
        ),
    };
    let style = match args.get("style") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_str()
                .filter(|s| matches!(*s, "normal" | "italic" | "oblique"))
                .ok_or_else(|| invalid("style"))?,
        ),
    };
    let marked: Vec<Value> = views
        .iter()
        .map(
            |view| match page::mark(host, view.id, family, weight, style) {
                Ok(count) => json!({ "view": view.id, "count": count }),
                Err(failure) => json!({
                    "view": view.id, "count": 0,
                    "error": failure.code().unwrap_or(codes::HOST_FAILED),
                }),
            },
        )
        .collect();
    Ok(json!({ "marked": marked }))
}

/// `export`: the inventory of the target's views written as `fonts.json` and `fonts.csv`.
///
/// # Errors
///
/// `plugins.error.invalid` for malformed arguments, the target's or the write's failure.
pub fn export(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let settings = Settings::read(host);
    let collected = collect(host, args, &settings)?;
    let mut inventory = inventory_json(&collected, &settings);
    inventory["exportedAt"] = json!(host.now_unix_seconds());
    let families = inventory::families(&collected);
    let json_text = serde_json::to_string_pretty(&inventory).unwrap_or_else(|_| "{}".to_owned());
    host.out_write(export::JSON_FILE, json_text.as_bytes())?;
    host.out_write(export::CSV_FILE, export::csv(&families).as_bytes())?;
    Ok(json!({
        "files": [
            format!(".rbrowser/out/fonts/{}", export::JSON_FILE),
            format!(".rbrowser/out/fonts/{}", export::CSV_FILE),
        ],
        "families": families.len(),
        "views": collected.views.len(),
    }))
}

/// The command `check`: the findings of every view reported as a findings run of the plugin (the
/// findings panel shows it); answers the count.
///
/// # Errors
///
/// The host's failure to report.
pub fn check_command(host: &mut dyn PluginHost) -> Result<Value, Failure> {
    let ctx = rb_plugin_api::CheckContext {
        view: None,
        url: None,
        rules: Vec::new(),
        args: String::new(),
    };
    let found = run::check(host, &ctx)?;
    host.report_findings(&found)?;
    Ok(json!({ "findings": found.len() }))
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
