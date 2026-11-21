//! A check run as a SARIF 2.1.0 log (OASIS): one `run` per check plugin, in the order the
//! plugins ran, each with its rules (`tool.driver.rules`), whether it ran through
//! (`invocations`) and its results. A finding goes to the run of its first reporting plugin;
//! its breakpoints, views, engines, page and element are in the result's `properties`, its
//! stable id in `partialFingerprints`, the page as the location's `artifactLocation.uri` and
//! the element as a logical location. Levels: error, warning, note (info).

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use rb_plugin_types::{CheckFailure, FindingsFile, MergedFinding, Severity};
use serde_json::{Map, Value, json};

use super::ExportContext;

/// The schema the log names.
pub const SCHEMA: &str = "https://json.schemastore.org/sarif-2.1.0.json";
/// The SARIF version.
pub const VERSION: &str = "2.1.0";
/// The key of a result's stable id in `partialFingerprints`.
pub const FINGERPRINT_KEY: &str = "rbrowserFindingId/v1";

/// The SARIF level of a severity.
pub const fn level(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "note",
    }
}

/// The log of `file`.
pub fn render(file: &FindingsFile, context: &ExportContext<'_>) -> Value {
    let runs: Vec<Value> = plugins_of(file)
        .iter()
        .map(|plugin| run_of(file, plugin, context))
        .collect();
    json!({ "$schema": SCHEMA, "version": VERSION, "runs": runs })
}

/// The plugins that ran, then any a finding names that the list misses, each once.
fn plugins_of(file: &FindingsFile) -> Vec<String> {
    let mut plugins = file.plugins.clone();
    for finding in &file.findings {
        if let Some(first) = finding.plugins.first()
            && !plugins.contains(first)
        {
            plugins.push(first.clone());
        }
    }
    plugins
}

fn run_of(file: &FindingsFile, plugin: &str, context: &ExportContext<'_>) -> Value {
    let findings: Vec<&MergedFinding> = file
        .findings
        .iter()
        .filter(|finding| finding.plugins.first().map(String::as_str) == Some(plugin))
        .collect();
    let mut rules: Vec<Value> = Vec::new();
    let mut index: BTreeMap<&str, usize> = BTreeMap::new();
    for finding in &findings {
        if let Entry::Vacant(slot) = index.entry(finding.rule_id.as_str()) {
            slot.insert(rules.len());
            rules.push(rule_of(finding));
        }
    }
    let results: Vec<Value> = findings
        .iter()
        .map(|finding| {
            let rule_index = index.get(finding.rule_id.as_str()).copied().unwrap_or(0);
            result_of(file, finding, rule_index, context)
        })
        .collect();
    let failures: Vec<&CheckFailure> = file
        .failures
        .iter()
        .filter(|failure| failure.plugin == plugin)
        .collect();
    let notifications: Vec<Value> = failures
        .iter()
        .map(|failure| {
            json!({
                "level": "error",
                "message": { "text": failure_text(failure, context) },
            })
        })
        .collect();
    let mut invocation = json!({ "executionSuccessful": failures.is_empty() });
    if !notifications.is_empty() {
        invocation["toolExecutionNotifications"] = Value::Array(notifications);
    }
    json!({
        "tool": { "driver": { "name": plugin, "rules": rules } },
        "automationDetails": { "id": format!("rbrowser/checks/{}", file.run_id) },
        "invocations": [invocation],
        "results": results,
    })
}

fn rule_of(finding: &MergedFinding) -> Value {
    let mut rule = json!({ "id": finding.rule_id });
    if let Some(help) = finding.help_url.as_deref().filter(|url| is_web(url)) {
        rule["helpUri"] = json!(help);
    }
    if let Some(shared) = &finding.shared_id {
        rule["properties"] = json!({ "sharedId": shared });
    }
    rule
}

fn result_of(
    file: &FindingsFile,
    finding: &MergedFinding,
    rule_index: usize,
    context: &ExportContext<'_>,
) -> Value {
    let element = finding.element.as_ref();
    let selector = element.and_then(|element| element.selector.as_deref());
    let path = element.and_then(|element| element.path.as_deref());
    let mut properties = Map::new();
    properties.insert("breakpoints".into(), json!(finding.breakpoints));
    properties.insert("views".into(), json!(finding.views));
    let names: Vec<String> = finding
        .views
        .iter()
        .map(|view| context.view_name(file, *view))
        .collect();
    properties.insert("viewNames".into(), json!(names));
    properties.insert("engines".into(), json!(context.engines_of(finding)));
    properties.insert("plugins".into(), json!(finding.plugins));
    for (key, value) in [
        ("url", finding.url.as_deref()),
        ("selector", selector),
        ("path", path),
    ] {
        if let Some(value) = value {
            properties.insert(key.into(), json!(value));
        }
    }
    let mut result = json!({
        "ruleId": finding.rule_id,
        "ruleIndex": rule_index,
        "level": level(finding.severity),
        "message": { "text": context.message(&finding.message) },
        "partialFingerprints": { FINGERPRINT_KEY: finding.id },
        "properties": properties,
    });
    let mut location = Map::new();
    if let Some(url) = &finding.url {
        location.insert(
            "physicalLocation".into(),
            json!({ "artifactLocation": { "uri": url } }),
        );
    }
    if let Some(element) = selector.or(path) {
        location.insert(
            "logicalLocations".into(),
            json!([{ "fullyQualifiedName": element, "kind": "element" }]),
        );
    }
    if !location.is_empty() {
        result["locations"] = json!([location]);
    }
    result
}

/// A failure as one sentence: the plugin and the reason's text (its code when it has none).
pub fn failure_text(failure: &CheckFailure, context: &ExportContext<'_>) -> String {
    let reason = failure.code.as_ref().map_or_else(String::new, |code| {
        let message = rb_plugin_types::FindingMessage {
            key: code.clone(),
            params: failure.params.clone(),
        };
        context.message(&message)
    });
    (context.text)(
        "plugin_failed",
        &[("plugin", &failure.plugin), ("reason", &reason)],
    )
}

/// Whether `url` is a web page (`http` or `https`): the only links an export gives.
pub fn is_web(url: &str) -> bool {
    let lower = url.trim_start().to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

#[cfg(test)]
#[path = "sarif_tests.rs"]
mod tests;
