//! The MCP tools `images_check` (the panel calls it too) and `images_mark` (the panel's hover),
//! and the command `check`.

use rb_plugin_api::{Failure, PluginHost, Severity, codes};
use serde_json::{Map, Value, json};

use crate::levels::RuleLevels;
use crate::mark;
use crate::page::{Image, PageImages};
use crate::rules;
use crate::run::{self, Measured, invalid};
use crate::settings::Settings;
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

/// One image of the answer: as measured, each issue with its message and the severity the
/// person's levels give it; an issue of a rule turned off left out.
fn image_json(view: u32, page: &PageImages, image: &Image, levels: &RuleLevels) -> Value {
    let issues: Vec<Value> = image
        .issues
        .iter()
        .filter_map(|issue| {
            let finding = levels.apply(rules::finding(view, &page.url, page.dpr, image, issue))?;
            let params: Map<String, Value> = finding
                .params
                .into_iter()
                .map(|(key, value)| (key, Value::String(value)))
                .collect();
            Some(json!({
                "rule": issue.rule, "variant": issue.variant,
                "severity": finding.severity.as_str(), "message": finding.message,
                "params": params,
            }))
        })
        .collect();
    let mut value = serde_json::to_value(image).unwrap_or(Value::Null);
    value["issues"] = Value::Array(issues);
    value
}

fn view_json(one: &Measured, levels: &RuleLevels, summary: &mut [u64; 3]) -> Value {
    let view = &one.view;
    let mut value = json!({
        "id": view.id, "label": view.label, "breakpoint": view.breakpoint, "engine": view.engine,
        "width": view.width, "height": view.height, "dpr": view.dpr,
    });
    match &one.page {
        Ok(page) => {
            let images: Vec<Value> = page
                .images
                .iter()
                .map(|image| image_json(view.id, page, image, levels))
                .collect();
            for issue in images
                .iter()
                .flat_map(|image| image["issues"].as_array().into_iter().flatten())
            {
                match Severity::parse(issue["severity"].as_str().unwrap_or_default()) {
                    Some(Severity::Error) => summary[0] += 1,
                    Some(Severity::Warning) => summary[1] += 1,
                    _ => summary[2] += 1,
                }
            }
            value["url"] = json!(page.url);
            value["dpr"] = json!(page.dpr);
            value["images"] = Value::Array(images);
            value["more"] = json!(page.more);
            value["lcpKnown"] = json!(page.lcp_known);
        }
        Err(failure) => {
            value["error"] = json!({
                "code": failure.code().unwrap_or(codes::HOST_FAILED), "params": failure.params,
            });
        }
    }
    value
}

/// The answer of a check: each view with its images and their issues, a count per severity,
/// and the settings the measure used.
#[must_use]
pub fn answer(measured: &[Measured], settings: &Settings, levels: &RuleLevels) -> Value {
    let mut summary = [0_u64; 3];
    let views: Vec<Value> = measured
        .iter()
        .map(|one| view_json(one, levels, &mut summary))
        .collect();
    json!({
        "views": views,
        "summary": { "error": summary[0], "warning": summary[1], "info": summary[2] },
        "settings": {
            "oversizeFactor": settings.oversize_factor,
            "undersizeFactor": settings.undersize_factor,
            "checkLcp": settings.check_lcp,
        },
        "cut": false,
    })
}

fn size(value: &Value) -> usize {
    value.to_string().len()
}

fn images_of(full: &mut Value) -> impl Iterator<Item = &mut Value> {
    full["views"]
        .as_array_mut()
        .into_iter()
        .flatten()
        .filter_map(|view| view.get_mut("images").and_then(Value::as_array_mut))
        .flatten()
}

/// `full` held to `budget` bytes: the raw srcset and sizes texts dropped first, then the images
/// without an issue (counted in the view's `hidden`); the images with an issue stay. `cut` says
/// so.
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
    for image in images_of(&mut full) {
        for key in ["srcset", "src", "natural"] {
            if let Some(object) = image.as_object_mut() {
                object.remove(key);
            }
        }
        if let Some(source) = image.get_mut("source").and_then(Value::as_object_mut) {
            source.remove("srcset");
        }
    }
    if size(&full) <= budget {
        return full;
    }
    for view in full["views"].as_array_mut().into_iter().flatten() {
        let hidden = view
            .get_mut("images")
            .and_then(Value::as_array_mut)
            .map(|images| {
                let before = images.len();
                images.retain(|image| {
                    image["issues"]
                        .as_array()
                        .is_some_and(|list| !list.is_empty())
                });
                before - images.len()
            });
        if let Some(hidden) = hidden {
            view["hidden"] = json!(hidden);
        }
    }
    full
}

/// MCP `images_check`: the images of the views `target` names (every view without one), each
/// measured at its view's density, with their issues; `budget` holds the answer to that many
/// bytes of JSON. Reports nothing to the findings panel (the command `check` does).
///
/// # Errors
///
/// A bad `target` or `budget`, or a view that is gone. A view that cannot be measured is
/// answered with its `error`.
pub fn check(host: &mut dyn PluginHost, args: &Map<String, Value>) -> Result<Value, Failure> {
    let budget = match args.get("budget") {
        None | Some(Value::Null) => None,
        Some(value) => Some(value.as_u64().ok_or_else(|| invalid("budget"))?),
    };
    let views = run::targets(host, args.get("target").unwrap_or(&Value::Null))?;
    let settings = Settings::read(host);
    let levels = RuleLevels::read(host);
    let measured = run::measure(host, views, &settings);
    Ok(fitted(answer(&measured, &settings, &levels), budget))
}

/// The command `check`: the views `target` names (the context menu's clicked view; every view
/// without one) measured, their findings into the findings panel; an open panel is told to load
/// again.
///
/// # Errors
///
/// A bad `target`, a view that is gone, or - for one view - its failure to be measured.
pub fn check_command(
    host: &mut dyn PluginHost,
    args: &Map<String, Value>,
) -> Result<Value, Failure> {
    let target = args.get("target").unwrap_or(&Value::Null);
    let views = run::targets(host, target)?;
    let settings = Settings::read(host);
    let measured = run::measure(host, views, &settings);
    if target.is_number()
        && let Some(Measured {
            page: Err(failure), ..
        }) = measured.first()
    {
        return Err(failure.clone());
    }
    let findings = run::findings(&measured, &[]);
    let reported = host.report_findings(&findings).is_ok();
    let views = measured.iter().filter(|one| one.page.is_ok()).count();
    Ok(json!({
        "views": views, "findings": findings.len(), "reported": reported,
        POST_KEY: [{ "target": "panel", "message": { "type": "checked", "panel": PANEL_ID } }],
    }))
}

/// MCP `images_mark`: an image marked and scrolled into view in the views named (the panel's
/// hover, in every view), or every mark cleared.
///
/// # Errors
///
/// `plugins.error.invalid` for malformed targets.
pub fn mark(host: &mut dyn PluginHost, args: &Map<String, Value>) -> Result<Value, Failure> {
    if args.get("clear").and_then(Value::as_bool) == Some(true) {
        for view in host.views()? {
            let _ = mark::clear(host, view.id);
        }
        return Ok(json!({ "cleared": true }));
    }
    let targets = mark::targets(args.get("targets").unwrap_or(&Value::Null))?;
    let known: Vec<_> = host.views()?.into_iter().map(|view| view.id).collect();
    let marked: Vec<Value> = targets
        .iter()
        .map(|(view, locator)| {
            if !known.contains(view) {
                return json!({ "view": view, "found": false, "error": codes::NOT_FOUND });
            }
            match mark::mark(host, *view, locator) {
                Ok(found) => json!({ "view": view, "found": found }),
                Err(failure) => json!({
                    "view": view, "found": false,
                    "error": failure.code().unwrap_or(codes::HOST_FAILED),
                }),
            }
        })
        .collect();
    Ok(json!({ "marked": marked }))
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
