//! The MCP tools and the panel's calls: `get` (the outline of the views, merged or per view),
//! `check_links` (the broken-link check of the current page), `mark` (a row marked in the views),
//! and the command `check` (everything, the links too, reported as findings).

use rb_plugin_api::{Failure, FailureKind, Finding, PluginHost, codes};
use serde_json::{Value, json};

use crate::axtree;
use crate::levels::RuleLevels;
use crate::links::{self, MAX_LINKS};
use crate::mark;
use crate::merge;
use crate::page::SECTIONS;
use crate::rules;
use crate::run::{self, Collected};
use crate::settings::Settings;
use crate::url::{MAX_URL_CHARS, without_fragment};

/// The section the accessibility tree is (from CDP, Chromium only).
pub const AXTREE: &str = "axtree";
/// The smallest and largest answer budget a caller may set, in bytes of JSON.
pub const MIN_BUDGET: usize = 4 * 1024;
pub const MAX_BUDGET: usize = 4 * 1024 * 1024;

pub(crate) fn invalid(reason: &str) -> Failure {
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

/// The answer budget asked for, in bytes of JSON; none without `budget`.
///
/// # Errors
///
/// `plugins.error.invalid` for a budget outside [`MIN_BUDGET`] to [`MAX_BUDGET`].
pub(crate) fn budget(args: &Value) -> Result<Option<usize>, Failure> {
    args.get("budget")
        .map(|value| {
            value
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .filter(|n| (MIN_BUDGET..=MAX_BUDGET).contains(n))
                .ok_or_else(|| invalid("budget"))
        })
        .transpose()
}

/// The sections asked for: the page's and whether the accessibility tree is one of them; every
/// section when none is named.
fn sections(args: &Value) -> Result<(Vec<String>, bool), Failure> {
    let list = match args.get("sections") {
        None => return Ok((SECTIONS.map(str::to_owned).to_vec(), true)),
        Some(list) => list.as_array().ok_or_else(|| invalid("sections"))?,
    };
    if list.is_empty() {
        return Ok((SECTIONS.map(str::to_owned).to_vec(), true));
    }
    let mut page = Vec::new();
    let mut tree = false;
    for name in list {
        match name.as_str() {
            Some(AXTREE) => tree = true,
            Some(name) if SECTIONS.contains(&name) => {
                if !page.iter().any(|known| known == name) {
                    page.push(name.to_owned());
                }
            }
            _ => return Err(invalid("sections")),
        }
    }
    Ok((page, tree))
}

/// A finding as the tools answer it; a reading-order finding's other element as `data`.
pub(crate) fn finding_json(finding: &Finding) -> Value {
    let mut value = json!({
        "rule": finding.rule,
        "severity": finding.severity.as_str(),
        "message": finding.message,
        "params": params_json(finding),
        "view": finding.view,
        "selector": finding.selector,
        "path": finding.path,
    });
    if let Some(data) = finding
        .data
        .as_deref()
        .and_then(|data| serde_json::from_str::<Value>(data).ok())
    {
        value["data"] = data;
    }
    value
}

fn params_json(finding: &Finding) -> Value {
    Value::Object(
        finding
            .params
            .iter()
            .map(|(key, value)| (key.clone(), Value::String(value.clone())))
            .collect(),
    )
}

/// `get`: the outline of the target's views. `sections` (default all, `axtree` included),
/// `shape` `merged` (rows with a cell per view, default) or `views` (each view's own lists),
/// `budget` (bytes of JSON the answer must fit; lists are cut and marked `cut`), `findingsOnly`
/// (the views and the findings of the sections, no lists: the panel's counts per tab). The
/// findings are every rule's but the link check's, those over the breakpoints among them, each
/// at the level the person's `checks.rules` gives its rule ([`RuleLevels`]).
///
/// # Errors
///
/// `plugins.error.invalid` for malformed arguments, the target's failure.
pub fn get(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let (page_sections, tree) = sections(args)?;
    let shape = args["shape"].as_str().unwrap_or("merged");
    if !matches!(shape, "merged" | "views") {
        return Err(invalid("shape"));
    }
    let budget = budget(args)?;
    let findings_only = match args.get("findingsOnly") {
        None => false,
        Some(value) => value.as_bool().ok_or_else(|| invalid("findingsOnly"))?,
    };
    let tree = tree && !findings_only;
    let settings = Settings::read(host);
    // The person's `checks.rules`, as the findings panel shows them (RB-091-06).
    let levels = RuleLevels::read(host);
    let views = run::targets(host, &args["target"])?;
    let collect_page = !page_sections.is_empty();
    let collected = if collect_page {
        run::collect(host, &views, &page_sections, &settings)
    } else {
        Collected::default()
    };
    let names: Vec<&str> = page_sections.iter().map(String::as_str).collect();
    let mut view_list: Vec<Value> = views
        .iter()
        .map(|view| {
            let mut entry = json!({
                "id": view.id, "label": view.label, "breakpoint": view.breakpoint,
                "engine": view.engine, "width": view.width, "url": view.url,
            });
            if let Some((_, code)) = collected.failures.iter().find(|(id, _)| *id == view.id) {
                entry["error"] = json!(code);
            }
            entry
        })
        .collect();
    let mut sections_out = serde_json::Map::new();
    let lists = collect_page && !findings_only;
    if shape == "merged" && lists {
        let outlines: Vec<_> = collected.outlines.iter().map(|(id, o)| (*id, o)).collect();
        for name in &names {
            sections_out.insert((*name).to_owned(), merge::section(name, &outlines));
        }
    } else if lists {
        for entry in &mut view_list {
            let id = entry["id"].as_u64().unwrap_or_default();
            if let Some(outline) = collected.outline(u32::try_from(id).unwrap_or_default()) {
                entry["outline"] = own_sections(outline, &names);
            }
        }
    }
    if tree {
        let mut trees = serde_json::Map::new();
        for view in &views {
            let value = match axtree::read(host, view.id) {
                Ok(tree) => serde_json::to_value(tree).unwrap_or(Value::Null),
                Err(failure) => json!({ "error": failure.code().unwrap_or(codes::HOST_FAILED) }),
            };
            trees.insert(view.id.to_string(), value);
        }
        sections_out.insert(AXTREE.to_owned(), json!({ "views": trees }));
    }
    let subjects: Vec<_> = collected.outlines.iter().map(|(id, _)| *id).collect();
    // The rules over the breakpoints compare the sections of `outline.diff.elements` asked for.
    let compared = settings.diff_sections(&page_sections);
    let across = if compared.is_empty() {
        Vec::new()
    } else {
        run::across(&[], &views, &collected.outlines, &subjects, &compared)
    };
    let findings: Vec<Value> = collected
        .outlines
        .iter()
        .flat_map(|(view, outline)| {
            rules::findings(&[], *view, outline, &page_sections, &[], &settings)
        })
        .chain(across)
        .filter_map(|finding| levels.apply(finding))
        .map(|finding| finding_json(&finding))
        .collect();
    let mut answer = json!({ "views": view_list, "findings": findings });
    if !sections_out.is_empty() {
        answer["sections"] = Value::Object(sections_out);
    }
    if let Some(budget) = budget {
        crate::budget::fit(&mut answer, budget);
    }
    Ok(answer)
}

fn own_sections(outline: &crate::page::PageOutline, names: &[&str]) -> Value {
    let full = serde_json::to_value(outline).unwrap_or(Value::Null);
    let mut own = serde_json::Map::new();
    for name in names {
        let list = if *name == "forms" { "fields" } else { name };
        own.insert((*name).to_owned(), full[list].clone());
    }
    own.insert("lang".to_owned(), full["lang"].clone());
    own.insert("truncated".to_owned(), full["truncated"].clone());
    Value::Object(own)
}

/// `check_links`: the broken-link check of the links the target's views show (`urls` narrows it
/// to those of them; any other URL is refused, so the tool fetches nothing the page does not
/// link). Off while `outline.linkCheck.enabled` is off.
///
/// # Errors
///
/// `plugins.error.invalid` for malformed arguments, the target's failure.
pub fn check_links(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let wanted: Option<Vec<String>> = match args.get("urls") {
        None => None,
        Some(list) => {
            let list = list.as_array().ok_or_else(|| invalid("urls"))?;
            if list.len() > MAX_LINKS {
                return Err(invalid("urls"));
            }
            Some(
                list.iter()
                    .map(|url| {
                        url.as_str()
                            .filter(|url| url.chars().count() <= MAX_URL_CHARS)
                            .map(without_fragment)
                            .ok_or_else(|| invalid("urls"))
                    })
                    .collect::<Result<_, _>>()?,
            )
        }
    };
    let settings = Settings::read(host);
    if !settings.link_check {
        return Ok(json!({ "enabled": false, "results": [] }));
    }
    let views = run::targets(host, &args["target"])?;
    let collected = run::collect(host, &views, &["links".to_owned()], &settings);
    let on_page: Vec<String> = links::distinct(&run::hrefs(&collected));
    let (urls, refused): (Vec<String>, Vec<String>) = match wanted {
        None => (on_page, Vec::new()),
        Some(wanted) => wanted.into_iter().partition(|url| on_page.contains(url)),
    };
    let wait_view = collected.outlines.first().map(|(id, _)| *id);
    let results = links::check(host, &settings, &urls, wait_view);
    Ok(json!({
        "enabled": true,
        "rps": settings.rps,
        "budgetMs": settings.timeout_ms,
        "results": results,
        "notOnPage": refused,
    }))
}

/// `mark`: `targets` marked in their views, or `clear` (`true`) the marks of every view.
///
/// # Errors
///
/// `plugins.error.invalid` for malformed targets.
pub fn mark(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    if args["clear"].as_bool() == Some(true) {
        let views = host.views()?;
        for view in views {
            let _ = mark::clear(host, view.id);
        }
        return Ok(json!({ "cleared": true }));
    }
    let targets = mark::targets(&args["targets"])?;
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

/// The command `check`: every section of every view, the links of each page once, reported as
/// a findings run of the plugin (the findings panel shows it); answers the counts.
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
    let findings = run::check(host, &ctx)?;
    host.report_findings(&findings)?;
    let broken = findings
        .iter()
        .filter(|finding| finding.rule == rules::LINK_BROKEN)
        .count();
    Ok(json!({ "findings": findings.len(), "brokenLinks": broken }))
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
