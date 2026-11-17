//! What the rules' tests share: outlines written as JSON, findings as short lines.

use rb_plugin_api::Finding;
use serde_json::{Value, json};

use crate::page::PageOutline;

/// An exposed, visible element at `path` (its selector the same).
pub fn at(path: &str) -> Value {
    json!({ "selector": path, "path": path, "visible": true, "hidden": false })
}

/// `entry` with `fields` added (or replaced).
pub fn with(mut entry: Value, fields: Value) -> Value {
    if let (Some(entry), Some(fields)) = (entry.as_object_mut(), fields.as_object()) {
        entry.extend(fields.clone());
    }
    entry
}

/// A heading of `level` with `text` at `path`.
pub fn heading(path: &str, level: u32, text: &str) -> Value {
    with(at(path), json!({ "level": level, "text": text }))
}

/// A landmark of `role` named `name` at `path`.
pub fn landmark(path: &str, role: &str, name: &str) -> Value {
    with(at(path), json!({ "role": role, "name": name }))
}

pub fn page(value: Value) -> PageOutline {
    PageOutline::parse(&value.to_string()).expect("outline")
}

/// `<rule> <path>` per finding.
pub fn short(findings: &[Finding]) -> Vec<String> {
    findings
        .iter()
        .map(|f| format!("{} {}", f.rule, f.path.as_deref().unwrap_or("-")))
        .collect()
}

/// The value of parameter `name` of `finding`.
pub fn param<'a>(finding: &'a Finding, name: &str) -> Option<&'a str> {
    finding
        .params
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}
