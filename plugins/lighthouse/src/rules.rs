//! Lighthouse's audits as the plugin's check rules: one rule per audit of the four categories,
//! from the table `data/rules.json` (written from the pinned Lighthouse by
//! `sidecar/src/modules/lighthouse/gen_rules.ts`, which also writes the rule titles into
//! `locales/en.json`). A rule's usual severity is an error when the audit counts towards a
//! category's score (a weight above 0), a warning when it does not.
//!
//! Lighthouse's audits run axe-core too, but Lighthouse names an element by its position among
//! all child nodes (`1,HTML,1,BODY,3,P`), not by the tag path the seo and a11y-axe plugins share
//! (`html > body > p:nth-of-type(2)`); so no rule claims a shared id, and the same missing alt
//! text found by both plugins stays two findings.

use std::sync::LazyLock;

use rb_plugin_api::{RuleInfo, Severity};
use serde::Deserialize;

use crate::key;

/// The audit table.
const RULES_JSON: &str = include_str!("../data/rules.json");

/// The categories, in Lighthouse's order.
pub const CATEGORIES: [&str; 4] = ["performance", "accessibility", "best-practices", "seo"];

/// One audit of the table.
#[derive(Clone, Debug, Deserialize)]
pub struct Audit {
    pub id: String,
    pub categories: Vec<String>,
    /// The highest weight of the audit in its categories.
    pub weight: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Table {
    lighthouse_version: String,
    audits: Vec<Audit>,
}

static TABLE: LazyLock<Table> = LazyLock::new(|| {
    serde_json::from_str(RULES_JSON).expect("data/rules.json is checked by its tests")
});

/// The Lighthouse version the table was written from.
#[must_use]
pub fn lighthouse_version() -> &'static str {
    &TABLE.lighthouse_version
}

/// Every audit of the table.
#[must_use]
pub fn all() -> &'static [Audit] {
    &TABLE.audits
}

/// The audit `id`, if the table has it.
#[must_use]
pub fn find(id: &str) -> Option<&'static Audit> {
    TABLE.audits.iter().find(|audit| audit.id == id)
}

/// Whether `category` is one of Lighthouse's four.
#[must_use]
pub fn is_category(category: &str) -> bool {
    CATEGORIES.contains(&category)
}

/// The usual severity of an audit of weight `weight`.
#[must_use]
pub fn usual_severity(weight: f64) -> Severity {
    if weight > 0.0 {
        Severity::Error
    } else {
        Severity::Warning
    }
}

/// The rules `check.describe` answers.
#[must_use]
pub fn describe() -> Vec<RuleInfo> {
    all()
        .iter()
        .map(|audit| RuleInfo {
            id: audit.id.clone(),
            title: key("rules", &audit.id),
            severity: usual_severity(audit.weight),
            shared_id: None,
            help_url: None,
        })
        .collect()
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
