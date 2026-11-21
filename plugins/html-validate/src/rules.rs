//! The rules the plugin offers: the table `runner/dist/rules.json` (written by
//! `runner/scripts/build.mjs` from the pinned html-validate) - html-validate's
//! `html-validate:standard` preset, standards conformance without style rules, plus
//! `missing-doctype` and `no-missing-references`, each with its configuration; the opt-in
//! `no-unknown-attributes`, configured `off` until a project's rule file switches it on; and
//! `nu-error`, `nu-warning`, `nu-info`, the Nu HTML Checker's message types, which have no rule
//! ids of their own. Which html-validate rules a run configures follows the rules the person left
//! on in `checks.rules`, the setting `html-validate.disabledRules` and a trusted project's rule
//! file (`crate::project`), which takes precedence over the setting and sets levels and options.

use std::sync::LazyLock;

use rb_plugin_api::{RuleInfo, Severity};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::key;
use crate::project::level_of;
use crate::settings::Settings;

/// The rule table the build wrote.
const RULES_JSON: &str = include_str!("../runner/dist/rules.json");

/// The prefix of the Nu HTML Checker's rules.
pub const NU_PREFIX: &str = "nu-";

/// One rule.
#[derive(Clone, Debug, Deserialize)]
pub struct Rule {
    pub id: String,
    pub severity: Severity,
    /// html-validate's configuration of the rule (`"error"`, `["error", {...}]`); `null` for the
    /// Nu rules.
    pub config: Value,
    pub url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Table {
    html_validate_version: String,
    rules: Vec<Rule>,
}

static TABLE: LazyLock<Table> = LazyLock::new(|| {
    serde_json::from_str(RULES_JSON).expect("runner/dist/rules.json is checked by its tests")
});

/// The bundled html-validate's version.
#[must_use]
pub fn html_validate_version() -> &'static str {
    &TABLE.html_validate_version
}

/// Every rule.
#[must_use]
pub fn all() -> &'static [Rule] {
    &TABLE.rules
}

/// Rule `id`, if the plugin offers it.
#[must_use]
pub fn find(id: &str) -> Option<&'static Rule> {
    all().iter().find(|rule| rule.id == id)
}

/// Whether `id` is one of the Nu HTML Checker's rules.
#[must_use]
pub fn is_nu(id: &str) -> bool {
    id.starts_with(NU_PREFIX)
}

/// The rules as `check.describe` answers them.
#[must_use]
pub fn describe() -> Vec<RuleInfo> {
    all()
        .iter()
        .map(|rule| RuleInfo {
            id: rule.id.clone(),
            title: key("rules", &rule.id),
            severity: rule.severity,
            shared_id: None,
            help_url: (!rule.url.is_empty()).then(|| rule.url.clone()),
        })
        .collect()
}

/// Whether a message of rule `id` is reported: the rule is in `enabled` (the rules the person
/// left on) and not switched off in the settings.
#[must_use]
pub fn wanted(id: &str, settings: &Settings, enabled: &[String]) -> bool {
    enabled.iter().any(|rule| rule == id) && !settings.disabled(id)
}

/// html-validate's `rules` configuration of a run: every wanted html-validate rule with its
/// configuration - the project's rule file's where it sets one, else the table's - whose level
/// is on and at or above the lowest severity; html-validate leaves every other rule off.
#[must_use]
pub fn builtin_config(settings: &Settings, enabled: &[String]) -> Map<String, Value> {
    all()
        .iter()
        .filter(|rule| !is_nu(&rule.id) && wanted(&rule.id, settings, enabled))
        .filter_map(|rule| {
            let config = settings.project.config(&rule.id).unwrap_or(&rule.config);
            let severity = level_of(config)?.severity()?;
            settings
                .keeps(severity)
                .then(|| (rule.id.clone(), config.clone()))
        })
        .collect()
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
