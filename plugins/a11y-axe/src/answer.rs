//! The runner's answer of one view (`Answer` in `runner/src/run.ts`) and the findings it makes:
//! one per rule and element, the rule id axe's own (`button-name`; the check runner puts the
//! plugin's id in front), the severity from the element's impact (critical and serious an error,
//! moderate a warning, minor information; a "needs review" result always information), the
//! message a key with axe's English help text as a parameter, the element's selector and its path
//! (what the stable id is made of), and the help page, impact, WCAG tags, HTML and failure summary
//! in `data`.

use rb_plugin_api::{Failure, FailureKind, Finding, Severity, ViewId};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{key, rules};

/// One node of a rule's result.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Node {
    pub selector: String,
    pub path: String,
    pub impact: Option<String>,
    pub html: String,
    pub summary: String,
}

/// One rule's result.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RuleResult {
    pub id: String,
    pub impact: Option<String>,
    pub help: String,
    pub help_url: String,
    pub tags: Vec<String>,
    pub nodes: Vec<Node>,
    pub omitted: u32,
}

/// The answer of one view.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Answer {
    pub axe_version: String,
    pub url: String,
    pub violations: Vec<RuleResult>,
    pub incomplete: Vec<RuleResult>,
    pub truncated: bool,
    /// Set instead of the rest when axe failed (an invalid selector) or did not load.
    pub error: Option<String>,
}

fn bad_answer(view: ViewId, reason: &str) -> Failure {
    Failure::coded(FailureKind::BadData, &key("errors", "bad_answer"))
        .with_param("view", view)
        .with_param("reason", reason)
}

/// The answer in `text`, the JSON `evaluate` gave back - the runner's object, or that object
/// as a JSON string when the host hands a value over as text.
///
/// # Errors
///
/// The text is no answer (`errors.bad_answer`), or axe failed in the view (`errors.axe_failed`).
pub fn parse(view: ViewId, text: &str) -> Result<Answer, Failure> {
    let value: Value =
        serde_json::from_str(text).map_err(|error| bad_answer(view, &error.to_string()))?;
    let value = match value {
        Value::String(inner) => {
            serde_json::from_str(&inner).map_err(|error| bad_answer(view, &error.to_string()))?
        }
        other => other,
    };
    if !value.is_object() {
        return Err(bad_answer(view, "the answer is no object"));
    }
    let answer: Answer =
        serde_json::from_value(value).map_err(|error| bad_answer(view, &error.to_string()))?;
    if let Some(error) = &answer.error {
        return Err(
            Failure::coded(FailureKind::Other, &key("errors", "axe_failed"))
                .with_param("view", view)
                .with_param("reason", error),
        );
    }
    Ok(answer)
}

fn finding(view: ViewId, url: &str, rule: &RuleResult, node: &Node, review: bool) -> Finding {
    let impact = node.impact.as_deref().or(rule.impact.as_deref());
    let severity = if review {
        Severity::Info
    } else {
        rules::severity_of(impact)
    };
    let message = if review { "incomplete" } else { "violation" };
    let wcag: Vec<&String> = rule
        .tags
        .iter()
        .filter(|tag| tag.starts_with("wcag"))
        .collect();
    let data = json!({
        "impact": impact,
        "wcag": wcag,
        "tags": rule.tags,
        "helpUrl": rule.help_url,
        "html": node.html,
        "summary": node.summary,
        "needsReview": review,
    });
    Finding {
        rule: rule.id.clone(),
        severity,
        message: key("findings", message),
        params: vec![
            ("help".to_owned(), rule.help.clone()),
            ("impact".to_owned(), impact.unwrap_or("minor").to_owned()),
            ("rule".to_owned(), rule.id.clone()),
        ],
        view: Some(view),
        url: (!url.is_empty()).then(|| url.to_owned()),
        selector: (!node.selector.is_empty()).then(|| node.selector.clone()),
        path: (!node.path.is_empty()).then(|| node.path.clone()),
        data: Some(data.to_string()),
    }
}

impl Answer {
    /// The findings of this answer from view `view`: every node of every violation, and of every
    /// "needs review" result when `incomplete`; only rules in `asked` (axe answers nothing else,
    /// but a finding of a rule the check did not run would be dropped by the host anyway).
    #[must_use]
    pub fn findings(&self, view: ViewId, asked: &[String], incomplete: bool) -> Vec<Finding> {
        let wanted = |rule: &RuleResult| asked.iter().any(|id| *id == rule.id);
        let violations = self
            .violations
            .iter()
            .filter(|rule| wanted(rule))
            .flat_map(|rule| rule.nodes.iter().map(move |node| (rule, node, false)));
        let review = self
            .incomplete
            .iter()
            .filter(|rule| incomplete && wanted(rule))
            .flat_map(|rule| rule.nodes.iter().map(move |node| (rule, node, true)));
        violations
            .chain(review)
            .map(|(rule, node, review)| finding(view, &self.url, rule, node, review))
            .collect()
    }
}

#[cfg(test)]
#[path = "answer_tests.rs"]
mod tests;
