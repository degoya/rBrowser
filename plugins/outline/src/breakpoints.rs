//! The outline across the breakpoints (RB-094-02): the MCP tools `outline_breakpoint_diff` (the
//! table element x breakpoint: the headings, landmarks and interactive elements some views of a
//! page show and others lack or hide) and `outline_reading_order` (the reading order against the
//! source and the focus order), the panel's tabs Breakpoints and Reading order, and the commands
//! that show those tabs. The findings are the check's (`checks_run` runs the same rules), each at
//! the level the person's `checks.rules` gives it.

use rb_plugin_api::{Failure, PluginHost, ViewId, ViewInfo};
use serde_json::{Value, json};

use crate::levels::RuleLevels;
use crate::page::PageOutline;
use crate::rules::{self, Peer};
use crate::run::{self, Collected};
use crate::settings::{self, DIFF_ELEMENTS, Settings};
use crate::tool::{self, finding_json, invalid};
use crate::{OPEN_KEY, PANEL_ID};

/// The sections the reading order reads.
const READING: [&str; 2] = ["headings", "focus"];

/// The kinds of element `args` names (`elements`), else those of `outline.diff.elements`.
fn elements(args: &Value, settings: &Settings) -> Result<Vec<String>, Failure> {
    let Some(list) = args.get("elements") else {
        return Ok(settings.diff_elements.clone());
    };
    let list = list.as_array().ok_or_else(|| invalid("elements"))?;
    let mut chosen: Vec<String> = Vec::new();
    for entry in list {
        let kind = entry
            .as_str()
            .filter(|kind| DIFF_ELEMENTS.contains(kind))
            .ok_or_else(|| invalid("elements"))?;
        if !chosen.iter().any(|known| known == kind) {
            chosen.push(kind.to_owned());
        }
    }
    Ok(chosen)
}

/// The views of `all` the answer names - those read and those that gave no outline - in the
/// grid's order, with why a view gave none.
fn view_list(
    all: &[ViewInfo],
    pool: &[(ViewId, PageOutline)],
    failures: &[(ViewId, String)],
) -> Vec<Value> {
    all.iter()
        .filter_map(|view| {
            let outline = pool.iter().find(|(id, _)| *id == view.id).map(|(_, o)| o);
            let failure = failures.iter().find(|(id, _)| *id == view.id);
            if outline.is_none() && failure.is_none() {
                return None;
            }
            let mut entry = json!({
                "id": view.id, "label": view.label, "breakpoint": view.breakpoint,
                "engine": view.engine, "width": view.width, "url": view.url,
            });
            if let Some(outline) = outline {
                entry["dir"] = json!(if outline.is_rtl() { "rtl" } else { "ltr" });
            }
            if let Some((_, code)) = failure {
                entry["error"] = json!(code);
            }
            Some(entry)
        })
        .collect()
}

/// `breakpoint_diff`: what the views of the target's pages do not all show. `target` (`"all"` or
/// a view id) names the views asked about, each compared with every Chromium view on its URL;
/// `elements` the kinds compared (default `outline.diff.elements`); `budget` as `get`'s. Answers
/// `{ elements, views, rows: [{ url, kind, label, selector, path, views, shown, missing }],
/// findings }`, a row for each element a view asked about lacks or hides.
///
/// # Errors
///
/// `plugins.error.invalid` for malformed arguments, the target's failure.
pub fn breakpoint_diff(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let settings = Settings::read(host);
    let kinds = elements(args, &settings)?;
    let budget = tool::budget(args)?;
    let levels = RuleLevels::read(host);
    let targets = run::targets(host, &args["target"])?;
    let all = host.views()?;
    let sections: Vec<String> = kinds
        .iter()
        .filter_map(|kind| settings::diff_section(kind.as_str()))
        .map(str::to_owned)
        .collect();
    if sections.is_empty() {
        return Ok(json!({ "elements": kinds, "views": [], "rows": [], "findings": [] }));
    }
    let Collected {
        outlines: mut pool,
        failures,
    } = run::collect(host, &targets, &sections, &settings);
    let subjects: Vec<ViewId> = pool.iter().map(|(id, _)| *id).collect();
    run::add_peers(host, &all, &mut pool, &sections);
    let mut urls: Vec<&str> = Vec::new();
    for view in all.iter().filter(|view| subjects.contains(&view.id)) {
        if !urls.contains(&view.url.as_str()) {
            urls.push(&view.url);
        }
    }
    let mut rows = Vec::new();
    for url in urls {
        let peers: Vec<Peer<'_>> = all
            .iter()
            .filter(|view| view.url == url)
            .filter_map(|view| {
                let (_, outline) = pool.iter().find(|(id, _)| *id == view.id)?;
                Some(Peer {
                    view: view.id,
                    label: &view.label,
                    outline,
                })
            })
            .collect();
        for difference in rules::differences(&peers, &sections) {
            if !difference
                .missing
                .iter()
                .any(|view| subjects.contains(view))
            {
                continue;
            }
            let mut row = serde_json::to_value(&difference).unwrap_or(Value::Null);
            row["url"] = json!(url);
            rows.push(row);
        }
    }
    let findings: Vec<Value> = run::across(&[], &all, &pool, &subjects, &sections)
        .into_iter()
        .filter_map(|finding| levels.apply(finding))
        .map(|finding| finding_json(&finding))
        .collect();
    let mut answer = json!({
        "elements": kinds,
        "views": view_list(&all, &pool, &failures),
        "rows": rows,
        "findings": findings,
    });
    if let Some(budget) = budget {
        crate::budget::fit(&mut answer, budget);
    }
    Ok(answer)
}

/// `reading_order`: per view of the target the reading order against the source order
/// (`reading-order`, WCAG 1.3.2) and the focus order against the visible order
/// (`focus-order-jump`, WCAG 2.4.3), with `outline.readingOrder.tolerancePx`. Answers `{ views:
/// [{ id, label, ..., dir }], findings }`; a reading-order finding names both elements and holds
/// the other one's `selector` and `path` in `data.other`.
///
/// # Errors
///
/// `plugins.error.invalid` for malformed arguments, the target's failure.
pub fn reading_order(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let settings = Settings::read(host);
    let budget = tool::budget(args)?;
    let levels = RuleLevels::read(host);
    let targets = run::targets(host, &args["target"])?;
    let all = host.views()?;
    let sections = READING.map(str::to_owned);
    let collected = run::collect(host, &targets, &sections, &settings);
    let enabled = [
        rules::READING_ORDER.to_owned(),
        rules::FOCUS_ORDER_JUMP.to_owned(),
    ];
    let findings: Vec<Value> = collected
        .outlines
        .iter()
        .flat_map(|(view, outline)| {
            rules::findings(&enabled, *view, outline, &sections, &[], &settings)
        })
        .filter_map(|finding| levels.apply(finding))
        .map(|finding| finding_json(&finding))
        .collect();
    let mut answer = json!({
        "views": view_list(&all, &collected.outlines, &collected.failures),
        "findings": findings,
    });
    if let Some(budget) = budget {
        crate::budget::fit(&mut answer, budget);
    }
    Ok(answer)
}

/// The command `breakpoint_diff` or `reading_order`: shows the outline panel and asks it for
/// that command's tab (a panel open already switches to it; one opening starts on its first).
#[must_use]
pub fn show(command: &str) -> Value {
    let tab = if command == "reading_order" {
        "order"
    } else {
        "breakpoints"
    };
    json!({
        OPEN_KEY: { "panel": PANEL_ID },
        "$post": [{ "target": "panel", "message": { "type": "show", "tab": tab } }],
    })
}

#[cfg(test)]
#[path = "breakpoints_tests.rs"]
mod tests;
