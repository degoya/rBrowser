//! The check: JS errors as findings (`check-plugin`).
//!
//! `js-error` - an uncaught exception, an error - and `console-error` - a `console.error` of the
//! page, a warning. A finding per group and view, all of one group with the same `path`
//! (`console/<group id>`): the host's check runner merges them into one finding with every view
//! that reported it.

use std::collections::BTreeMap;

use rb_plugin_api::{Finding, RuleInfo, Severity, ViewId};
use serde_json::json;

use crate::feed::{Kind, Level};
use crate::group::Group;
use crate::normalize::cut;
use crate::{PLUGIN_ID, key};

/// An uncaught exception.
pub const JS_ERROR: &str = "js-error";
/// The page called `console.error`.
pub const CONSOLE_ERROR: &str = "console-error";
/// The longest message text a finding quotes.
pub const MAX_QUOTE_CHARS: usize = 300;

/// The rules `describe` answers.
#[must_use]
pub fn rules() -> Vec<RuleInfo> {
    vec![
        RuleInfo {
            id: JS_ERROR.to_owned(),
            title: key("rules.js_error"),
            severity: Severity::Error,
            shared_id: None,
            help_url: None,
        },
        RuleInfo {
            id: CONSOLE_ERROR.to_owned(),
            title: key("rules.console_error"),
            severity: Severity::Warning,
            shared_id: None,
            help_url: None,
        },
    ]
}

/// The rule `group` breaks, with its severity and message key.
fn rule(group: &Group) -> Option<(&'static str, Severity, &'static str)> {
    match (group.kind, group.level) {
        (Kind::Exception, _) => Some((JS_ERROR, Severity::Error, "findings.js_error")),
        (Kind::Console, Level::Error) => {
            Some((CONSOLE_ERROR, Severity::Warning, "findings.console_error"))
        }
        _ => None,
    }
}

/// The findings of `groups` among the rules `wanted` (every rule when empty), for `view` only
/// when one is named; `urls` the page URL of each view.
#[must_use]
pub fn findings(
    groups: &[Group],
    view: Option<ViewId>,
    wanted: &[String],
    urls: &BTreeMap<ViewId, String>,
) -> Vec<Finding> {
    let mut out = Vec::new();
    for group in groups {
        let Some((rule, severity, message)) = rule(group) else {
            continue;
        };
        if !wanted.is_empty() && !wanted.iter().any(|id| id == rule) {
            continue;
        }
        let source = group.source().unwrap_or_else(|| "?".to_owned());
        for (&reporting, &count) in &group.views {
            if view.is_some_and(|view| view != reporting) {
                continue;
            }
            out.push(Finding {
                rule: rule.to_owned(),
                severity,
                message: format!("plugin.{PLUGIN_ID}.{message}"),
                params: vec![
                    ("text".to_owned(), cut(&group.text, MAX_QUOTE_CHARS)),
                    ("source".to_owned(), cut(&source, MAX_QUOTE_CHARS)),
                ],
                view: Some(reporting),
                url: urls.get(&reporting).cloned().filter(|url| !url.is_empty()),
                selector: None,
                path: Some(format!("console/{}", group.id)),
                data: Some(
                    json!({
                        "text": group.text,
                        "source": group.source(),
                        "count": count,
                        "stack": group.stack,
                    })
                    .to_string(),
                ),
            });
        }
    }
    out
}

#[cfg(test)]
#[path = "findings_tests.rs"]
mod tests;
