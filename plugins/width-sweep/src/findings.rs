//! The sweep's rules and its findings: one rule per kind of break, one finding per issue and
//! width range, its message "700-740 px: ..." with the element's selector (and for an overlap
//! the other element's), its `data` the range and the width in its middle - the width to look
//! at it.

use rb_plugin_api::check::MAX_FINDINGS;
use rb_plugin_api::{Finding, RuleInfo, Severity, ViewInfo};
use serde_json::json;

use crate::key;
use crate::plan::Range;
use crate::settings::KINDS;

/// The rule of each kind, in the order of [`KINDS`].
#[must_use]
pub fn describe() -> Vec<RuleInfo> {
    KINDS
        .iter()
        .map(|kind| RuleInfo {
            id: (*kind).to_owned(),
            title: key(&format!("rules.{kind}")),
            severity: severity(kind),
            shared_id: None,
            help_url: None,
        })
        .collect()
}

/// An overflow scrolls the whole page sideways; cut text and overlaps are local.
fn severity(kind: &str) -> Severity {
    if kind == "overflow" {
        Severity::Error
    } else {
        Severity::Warning
    }
}

/// The findings of `ranges` found in `view`, the narrowest widths first.
#[must_use]
pub fn findings(ranges: &[Range], view: &ViewInfo) -> Vec<Finding> {
    let mut ordered: Vec<&Range> = ranges.iter().collect();
    ordered.sort_by(|a, b| (a.from, a.to, &a.issue).cmp(&(b.from, b.to, &b.issue)));
    ordered
        .into_iter()
        .take(MAX_FINDINGS)
        .map(|range| finding(range, view))
        .collect()
}

fn finding(range: &Range, view: &ViewInfo) -> Finding {
    let issue = &range.issue;
    let mut params = vec![
        ("from".to_owned(), range.from.to_string()),
        ("to".to_owned(), range.to.to_string()),
        ("selector".to_owned(), issue.selector.clone()),
    ];
    if let Some(other) = &issue.other {
        params.push(("other".to_owned(), other.clone()));
    }
    let data = json!({
        "kind": issue.kind,
        "from": range.from,
        "to": range.to,
        "width": range.from + (range.to - range.from) / 2,
    });
    Finding {
        rule: issue.kind.clone(),
        severity: severity(&issue.kind),
        message: key(&format!("findings.{}", issue.kind)),
        params,
        view: Some(view.id),
        url: (!view.url.is_empty()).then(|| view.url.clone()),
        selector: Some(issue.selector.clone()),
        path: (!issue.path.is_empty()).then(|| issue.path.clone()),
        data: Some(data.to_string()),
    }
}

#[cfg(test)]
#[path = "findings_tests.rs"]
mod tests;
