//! The rules (group `coverage`) and the findings of a measurement, comparing the views:
//!
//! - `unused-file` (warning): a file of at least `minBytes` with more than `unusedPercent` unused
//!   in every view that loads it;
//! - `rule-partial` (info): a CSS rule used in some of the measured views that load its sheet
//!   but not in all - the breakpoints that use it are listed;
//! - `function-unused` (info, interaction runs only): a JavaScript function no view executed.
//!
//! A finding points at the file (`path` its URL; a rule or function with `#<start>-<end>`) on
//! the page's URL, so the check runner joins the same finding of every view into one.

use std::collections::BTreeMap;

use rb_plugin_api::{Finding, RuleInfo, Severity, ViewId};
use serde_json::json;

use crate::css_rules;
use crate::key;
use crate::model::{FileRun, Kind, Mode, Run, ViewRun};
use crate::settings::Settings;

pub const UNUSED_FILE: &str = "unused-file";
pub const RULE_PARTIAL: &str = "rule-partial";
pub const FUNCTION_UNUSED: &str = "function-unused";

/// The most `rule-partial` and `function-unused` findings of one run, each.
pub const MAX_DETAIL_FINDINGS: usize = 500;

const RULES: &[(&str, Severity)] = &[
    (UNUSED_FILE, Severity::Warning),
    (RULE_PARTIAL, Severity::Info),
    (FUNCTION_UNUSED, Severity::Info),
];

/// The rules of `check.describe`.
#[must_use]
pub fn describe() -> Vec<RuleInfo> {
    RULES
        .iter()
        .map(|(id, severity)| RuleInfo {
            id: (*id).to_owned(),
            title: key(&format!("rules.{id}")),
            severity: *severity,
            shared_id: None,
            help_url: None,
        })
        .collect()
}

fn wants(enabled: &[String], id: &str) -> bool {
    enabled.is_empty() || enabled.iter().any(|rule| rule == id)
}

fn severity(id: &str) -> Severity {
    RULES
        .iter()
        .find(|(rule, _)| *rule == id)
        .map_or(Severity::Info, |(_, severity)| *severity)
}

/// A page URL without its fragment: the one every view's finding names.
fn page(url: &str) -> String {
    url.split_once('#').map_or(url, |(page, _)| page).to_owned()
}

/// The views of `run` that have `file`, with their copy of it.
fn holders<'a>(views: &[&'a ViewRun], file: &FileRun) -> Vec<(&'a ViewRun, &'a FileRun)> {
    views
        .iter()
        .filter_map(|view| {
            view.files
                .iter()
                .find(|known| known.same(file))
                .map(|known| (*view, known))
        })
        .collect()
}

struct Out<'a> {
    enabled: &'a [String],
    only: Option<ViewId>,
    findings: Vec<Finding>,
}

impl Out<'_> {
    /// A finding of `id` for each of `views` (the one asked for alone when the run names one).
    fn emit(
        &mut self,
        id: &str,
        views: &[&ViewRun],
        path: &str,
        selector: Option<String>,
        params: Vec<(&str, String)>,
        data: serde_json::Value,
    ) {
        if !wants(self.enabled, id) {
            return;
        }
        for view in views {
            if self.only.is_some_and(|only| only != view.view) {
                continue;
            }
            self.findings.push(Finding {
                rule: id.to_owned(),
                severity: severity(id),
                message: key(&format!("findings.{id}")),
                params: params
                    .iter()
                    .map(|(name, value)| ((*name).to_owned(), value.clone()))
                    .collect(),
                view: Some(view.view),
                url: Some(page(&view.url)),
                selector: selector.clone(),
                path: Some(path.to_owned()),
                data: Some(data.to_string()),
            });
        }
    }
}

fn names(views: &[&ViewRun]) -> String {
    let mut names: Vec<&str> = views.iter().map(|view| view.name()).collect();
    names.dedup();
    names.join(", ")
}

/// Every distinct file of the measured views, in the order they first appear.
fn distinct<'a>(views: &[&'a ViewRun]) -> Vec<&'a FileRun> {
    let mut files: Vec<&FileRun> = Vec::new();
    for view in views {
        for file in &view.files {
            if !files.iter().any(|known| known.same(file)) {
                files.push(file);
            }
        }
    }
    files
}

fn unused_file(out: &mut Out<'_>, views: &[&ViewRun], file: &FileRun, settings: &Settings) {
    let held = holders(views, file);
    let all_unused = held.iter().all(|(_, copy)| {
        copy.total >= settings.min_bytes && copy.unused_percent() > settings.unused_percent as f64
    });
    if held.is_empty() || !all_unused {
        return;
    }
    let least = held
        .iter()
        .map(|(_, copy)| copy.unused_percent())
        .fold(f64::INFINITY, f64::min);
    let total = held.iter().map(|(_, copy)| copy.total).max().unwrap_or(0);
    let per_view: Vec<_> = held
        .iter()
        .map(|(view, copy)| json!({ "view": view.view, "breakpoint": view.name(), "total": copy.total, "used": copy.used }))
        .collect();
    let at: Vec<&ViewRun> = held.iter().map(|(view, _)| *view).collect();
    out.emit(
        UNUSED_FILE,
        &at,
        &file.url,
        None,
        vec![
            ("file", file.url.clone()),
            ("kind", file.kind.as_str().to_owned()),
            ("total", total.to_string()),
            ("percent", format!("{least}")),
        ],
        json!({ "kind": file.kind.as_str(), "views": per_view }),
    );
}

fn rule_partial(
    out: &mut Out<'_>,
    views: &[&ViewRun],
    file: &FileRun,
    texts: &BTreeMap<String, String>,
    budget: &mut usize,
) {
    let held = holders(views, file);
    if held.len() < 2 {
        return;
    }
    // Every rule any view used, by its range.
    let mut ranges: Vec<(u32, u32)> = held
        .iter()
        .flat_map(|(_, copy)| copy.rules.iter().copied())
        .collect();
    ranges.sort_unstable();
    ranges.dedup();
    let text = file.source.as_ref().and_then(|key| texts.get(key));
    for (start, end) in ranges {
        if *budget == 0 {
            return;
        }
        let using: Vec<&ViewRun> = held
            .iter()
            .filter(|(_, copy)| copy.rules.binary_search(&(start, end)).is_ok())
            .map(|(view, _)| *view)
            .collect();
        if using.len() == held.len() {
            continue;
        }
        let not: Vec<&ViewRun> = held
            .iter()
            .map(|(view, _)| *view)
            .filter(|view| !using.iter().any(|known| known.view == view.view))
            .collect();
        *budget -= 1;
        let selector = text.map(|text| {
            let head = css_rules::slice(text, start, end);
            let head = head.split('{').next().unwrap_or_default();
            head.split_whitespace().collect::<Vec<_>>().join(" ")
        });
        out.emit(
            RULE_PARTIAL,
            &using,
            &format!("{}#{start}-{end}", file.url),
            // No `selector`: the findings panel would look for an element with it.
            None,
            vec![
                ("selector", selector.unwrap_or_default()),
                ("file", file.url.clone()),
                ("breakpoints", names(&using)),
                ("unused", names(&not)),
            ],
            json!({ "start": start, "end": end,
                    "usedIn": using.iter().map(|view| view.view).collect::<Vec<_>>(),
                    "unusedIn": not.iter().map(|view| view.view).collect::<Vec<_>>() }),
        );
    }
}

fn function_unused(out: &mut Out<'_>, views: &[&ViewRun], file: &FileRun, budget: &mut usize) {
    let held = holders(views, file);
    let at: Vec<&ViewRun> = held.iter().map(|(view, _)| *view).collect();
    for function in &file.functions {
        if *budget == 0 {
            return;
        }
        let ran = held.iter().any(|(_, copy)| {
            copy.functions.iter().any(|known| {
                known.start == function.start && known.end == function.end && known.executed
            })
        });
        if ran {
            continue;
        }
        *budget -= 1;
        let name = if function.name.is_empty() {
            "(anonymous)".to_owned()
        } else {
            function.name.clone()
        };
        out.emit(
            FUNCTION_UNUSED,
            &at,
            &format!("{}#{}-{}", file.url, function.start, function.end),
            None,
            vec![
                ("name", name),
                ("file", file.url.clone()),
                ("offset", function.start.to_string()),
            ],
            json!({ "start": function.start, "end": function.end }),
        );
    }
}

/// The findings of `run` under `settings` for the rules `enabled` (empty: all); only `view`'s
/// when one is named. `texts` holds the stored sheets by source key, for the selectors.
#[must_use]
pub fn findings(
    run: &Run,
    settings: &Settings,
    enabled: &[String],
    view: Option<ViewId>,
    texts: &BTreeMap<String, String>,
) -> Vec<Finding> {
    let views: Vec<&ViewRun> = run.measured().collect();
    let mut out = Out {
        enabled,
        only: view,
        findings: Vec::new(),
    };
    let mut partial = MAX_DETAIL_FINDINGS;
    let mut functions = MAX_DETAIL_FINDINGS;
    for file in distinct(&views) {
        unused_file(&mut out, &views, file, settings);
        if file.kind == Kind::Css {
            rule_partial(&mut out, &views, file, texts, &mut partial);
        } else if run.mode == Mode::Interaction {
            function_unused(&mut out, &views, file, &mut functions);
        }
    }
    out.findings
}

#[cfg(test)]
#[path = "findings_tests.rs"]
mod tests;
