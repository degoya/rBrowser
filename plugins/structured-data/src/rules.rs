//! The rules of the structured data check (the host shows them as `structured-data.<rule>`) and
//! the findings made of the validation's issues.
//!
//! A rule's title is `plugin.structured-data.rules.<id>`, a finding's message
//! `plugin.structured-data.findings.<id>`. A finding's `selector` is the element its node is
//! written in (the `<script>` of a JSON-LD block, a Microdata or RDFa item's element), its `path`
//! the node's place with the property (`structured-data:jsonld/0#/@graph/1/headline`), so two
//! findings of one rule in one block stay two; its `data` names the panel and the node
//! (`{"panel": "structured-data", "node": "n3"}`).

use rb_plugin_api::{Finding, RuleInfo, Severity, ViewId};
use serde_json::json;

use crate::PANEL_ID;
use crate::validate::Issue;

/// The longest message parameter, in characters (the host takes 256).
pub const MAX_PARAM_CHARS: usize = 200;

pub const JSONLD_INVALID: &str = "jsonld.invalid";
pub const JSONLD_CONTEXT: &str = "jsonld.context";
pub const TYPE_MISSING: &str = "type.missing";
pub const TYPE_UNKNOWN: &str = "type.unknown";
pub const PROPERTY_UNKNOWN: &str = "property.unknown";
pub const PROPERTY_DOMAIN: &str = "property.domain";
pub const PROPERTY_RANGE: &str = "property.range";
pub const PROPERTY_SUPERSEDED: &str = "property.superseded";
pub const RICH_REQUIRED: &str = "rich.required";
pub const RICH_REQUIRED_ANY: &str = "rich.required_any";
pub const RICH_RECOMMENDED: &str = "rich.recommended";

/// Google Search's gallery of rich results: where each type's needs are described.
const RICH_HELP: &str =
    "https://developers.google.com/search/docs/appearance/structured-data/search-gallery";
/// schema.org's data model: types, properties and their expected values.
const SCHEMA_HELP: &str = "https://schema.org/docs/datamodel.html";

/// One rule of `describe`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    pub id: &'static str,
    pub severity: Severity,
}

const fn rule(id: &'static str, severity: Severity) -> Rule {
    Rule { id, severity }
}

use Severity::{Error, Info, Warning};

/// Every rule, in the order the panel lists them.
pub const RULES: &[Rule] = &[
    rule(JSONLD_INVALID, Error),
    rule(JSONLD_CONTEXT, Warning),
    rule(TYPE_MISSING, Warning),
    rule(TYPE_UNKNOWN, Error),
    rule(PROPERTY_UNKNOWN, Error),
    rule(PROPERTY_DOMAIN, Warning),
    rule(PROPERTY_RANGE, Warning),
    rule(PROPERTY_SUPERSEDED, Info),
    rule(RICH_REQUIRED, Error),
    rule(RICH_REQUIRED_ANY, Error),
    rule(RICH_RECOMMENDED, Warning),
];

/// The rules as `check.describe` answers them.
#[must_use]
pub fn describe() -> Vec<RuleInfo> {
    RULES
        .iter()
        .map(|rule| RuleInfo {
            id: rule.id.to_owned(),
            title: crate::key(&format!("rules.{}", rule.id)),
            severity: rule.severity,
            shared_id: None,
            help_url: Some(
                if rule.id.starts_with("rich.") {
                    RICH_HELP
                } else {
                    SCHEMA_HELP
                }
                .to_owned(),
            ),
        })
        .collect()
}

/// The severity of `rule`; `Info` for one not described.
#[must_use]
pub fn severity_of(rule: &str) -> Severity {
    RULES
        .iter()
        .find(|known| known.id == rule)
        .map_or(Info, |known| known.severity)
}

fn cut(text: &str) -> String {
    text.chars().take(MAX_PARAM_CHARS).collect()
}

/// What a run turns into findings.
#[derive(Clone, Copy, Debug)]
pub struct Scope<'a> {
    /// The rules the run asks for (`check-context.rules`); every rule when empty.
    pub enabled: &'a [String],
    /// The mildest severity reported (`structured-data.minSeverity`).
    pub min: Severity,
    pub view: Option<ViewId>,
    pub url: &'a str,
}

impl Scope<'_> {
    /// Whether an issue of `rule` becomes a finding.
    #[must_use]
    pub fn wants(&self, rule: &str) -> bool {
        (self.enabled.is_empty() || self.enabled.iter().any(|id| id == rule))
            && severity_of(rule) >= self.min
    }
}

/// The findings of `issues` the scope wants.
#[must_use]
pub fn findings(issues: &[Issue], scope: &Scope<'_>) -> Vec<Finding> {
    issues
        .iter()
        .filter(|issue| scope.wants(issue.rule))
        .map(|issue| Finding {
            rule: issue.rule.to_owned(),
            severity: severity_of(issue.rule),
            message: crate::key(&format!("findings.{}", issue.rule)),
            params: issue
                .params
                .iter()
                .map(|(key, value)| ((*key).to_owned(), cut(value)))
                .collect(),
            view: scope.view,
            url: Some(scope.url.to_owned()),
            selector: Some(issue.element.clone()),
            path: Some(format!("{PANEL_ID}:{}", issue.place)),
            data: Some(json!({ "panel": PANEL_ID, "node": issue.node }).to_string()),
        })
        .collect()
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
