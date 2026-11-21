//! The person's levels of a check plugin's own rules (`checks.rules.<plugin>.<rule>`,
//! RB-091-06). The host answers them through `settings.get` ([`RULE_LEVELS_SETTING`]), read only;
//! a plugin's panel and MCP tools show each finding with the severity the findings run gives it, a
//! rule turned off not at all. Read at each call, like the settings. Once the copies of
//! `plugins/outline`, `plugins/responsive-images` and `plugins/security-headers` (RB-094-11).

use std::collections::BTreeMap;

use rb_plugin_api::{Finding, PluginHost, RULE_LEVELS_SETTING, Severity};
use serde_json::Value;

/// By rule id: the severity its findings get, `None` for a rule turned off.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuleLevels(BTreeMap<String, Option<Severity>>);

impl RuleLevels {
    /// The levels the host holds; none (or a malformed answer) is no override.
    pub fn read(host: &mut dyn PluginHost) -> Self {
        host.setting(RULE_LEVELS_SETTING)
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .map(|value| Self::from_value(&value))
            .unwrap_or_default()
    }

    /// The levels of `value`, an object of rule id to `off` or a severity; anything else is
    /// left out.
    #[must_use]
    pub fn from_value(value: &Value) -> Self {
        let levels = value
            .as_object()
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|(rule, level)| {
                        let level = match level.as_str()? {
                            "off" => None,
                            name => Some(Severity::parse(name)?),
                        };
                        Some((rule.clone(), level))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self(levels)
    }

    /// Whether no rule has a level of its own.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// `finding` with its rule's level: the severity set, `None` for a rule turned off.
    #[must_use]
    pub fn apply(&self, mut finding: Finding) -> Option<Finding> {
        match self.0.get(&finding.rule) {
            None => Some(finding),
            Some(None) => None,
            Some(Some(severity)) => {
                finding.severity = *severity;
                Some(finding)
            }
        }
    }
}

#[cfg(test)]
#[path = "levels_tests.rs"]
mod tests;
