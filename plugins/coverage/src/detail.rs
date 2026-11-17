//! One file of the last measurement in detail, a window at a time (`coverage_file`, the panel's
//! detail): per view its used segments in the window; for CSS the stored text of the window and
//! its rules with the views that use each; for JavaScript its functions with the views that ran
//! each (no source: reading a script's text would need the `Debugger` domain).

use rb_plugin_api::{Failure, FailureKind, PluginHost, codes};
use serde_json::{Value, json};

use crate::css_rules::{self, units};
use crate::model::{Kind, Run, Status};
use crate::ranges::{Segment, overlaps};
use crate::store;

/// The default and the largest window, in UTF-16 units.
pub const WINDOW: u32 = 8_000;
pub const MAX_WINDOW: u32 = 16_000;
/// The most rules or functions one window lists.
pub const MAX_ITEMS: usize = 400;

fn invalid(reason: &str) -> Failure {
    Failure::coded(FailureKind::BadData, codes::INVALID).with_param("reason", reason)
}

fn clip(segments: &[Segment], from: u32, to: u32) -> Vec<Segment> {
    segments
        .iter()
        .filter(|(start, end)| *start < to && from < *end)
        .map(|(start, end)| ((*start).max(from), (*end).min(to)))
        .collect()
}

fn number(args: &Value, key: &str, default: u32, max: u32) -> u32 {
    args[key]
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .map_or(default, |value| value.min(max))
}

/// The window `args` names (`url`, optional `kind`, `offset`, `length`) of the file in `run`.
///
/// # Errors
///
/// `plugins.error.invalid` without a `url`, `plugins.error.not_found` for a file the run does
/// not have.
pub fn file(host: &mut dyn PluginHost, run: &Run, args: &Value) -> Result<Value, Failure> {
    let url = args["url"]
        .as_str()
        .filter(|url| !url.is_empty())
        .ok_or_else(|| invalid("url"))?;
    let kind = match args["kind"].as_str() {
        Some("js") => Some(Kind::Js),
        Some("css") => Some(Kind::Css),
        _ => None,
    };
    let copies: Vec<_> = run
        .views
        .iter()
        .filter(|view| view.status == Status::Measured)
        .filter_map(|view| {
            view.files
                .iter()
                .find(|file| file.url == url && kind.is_none_or(|kind| file.kind == kind))
                .map(|file| (view, file))
        })
        .collect();
    let Some((_, first)) = copies.first() else {
        return Err(Failure::coded(FailureKind::NotFound, codes::NOT_FOUND).with_param("url", url));
    };
    let total = copies.iter().map(|(_, file)| file.total).max().unwrap_or(0);
    let offset = number(args, "offset", 0, u32::MAX);
    let length = number(args, "length", WINDOW, MAX_WINDOW).max(1);
    let to = offset.saturating_add(length);
    let views: Vec<Value> = copies
        .iter()
        .map(|(view, file)| {
            json!({ "view": view.view, "label": view.label, "breakpoint": view.breakpoint,
                    "total": file.total, "used": file.used, "unusedPercent": file.unused_percent(),
                    "segments": clip(&file.segments, offset, to) })
        })
        .collect();
    let mut answer = json!({
        "url": url, "kind": first.kind, "inline": first.inline, "total": total,
        "offset": offset, "length": length,
        "next": if u64::from(to) < total { json!(to) } else { Value::Null },
        "views": views, "text": Value::Null,
    });
    if first.kind == Kind::Css {
        let text = first
            .source
            .as_deref()
            .and_then(|key| store::read_source(host, key));
        if let Some(text) = text {
            let rules: Vec<Value> = css_rules::rules(&text)
                .into_iter()
                .filter(|rule| rule.start < to && offset <= rule.start)
                .take(MAX_ITEMS)
                .map(|rule| {
                    let used: Vec<u32> = copies
                        .iter()
                        .filter(|(_, file)| overlaps(&file.segments, rule.body, rule.end))
                        .map(|(view, _)| view.view)
                        .collect();
                    json!({ "prelude": rule.prelude, "start": rule.start, "end": rule.end, "usedIn": used })
                })
                .collect();
            answer["text"] = json!(css_rules::slice(&text, offset, to));
            answer["textTotal"] = json!(units(&text));
            answer["rules"] = Value::Array(rules);
        }
    } else {
        let mut functions: Vec<Value> = Vec::new();
        for function in &first.functions {
            if function.start < offset || function.start >= to || functions.len() >= MAX_ITEMS {
                continue;
            }
            let ran: Vec<u32> = copies
                .iter()
                .filter(|(_, file)| {
                    file.functions.iter().any(|known| {
                        known.start == function.start && known.end == function.end && known.executed
                    })
                })
                .map(|(view, _)| view.view)
                .collect();
            functions.push(json!({ "name": function.name, "start": function.start,
                                   "end": function.end, "executedIn": ran }));
        }
        answer["functions"] = Value::Array(functions);
    }
    Ok(answer)
}

#[cfg(test)]
#[path = "detail_tests.rs"]
mod tests;
