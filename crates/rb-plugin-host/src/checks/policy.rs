//! The settings of the section Checks > Findings and what they decide about a finding:
//! `checks.parallel`, `checks.keepRuns`, `checks.rules.<plugin>.<rule>` (`off` or a severity) and
//! `checks.ignore` (glob patterns for the rule id, the URL and the selector).

use std::collections::BTreeMap;

use globset::{Glob, GlobMatcher};
use rb_plugin_types::Severity;
use serde::{Deserialize, Serialize};

/// Check runs at once by default: plugins times views.
pub const DEFAULT_PARALLEL: usize = 4;
/// The most check runs at once.
pub const MAX_PARALLEL: usize = 16;
/// Runs kept under `.rbrowser/out/checks/` by default.
pub const DEFAULT_KEEP_RUNS: usize = 10;
/// The most runs kept.
pub const MAX_KEEP_RUNS: usize = 100;

/// What `checks.rules.<id>` sets: the rule off, or the severity of its findings.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleLevel {
    Off,
    Info,
    Warning,
    Error,
}

impl RuleLevel {
    fn severity(self) -> Option<Severity> {
        match self {
            Self::Off => None,
            Self::Info => Some(Severity::Info),
            Self::Warning => Some(Severity::Warning),
            Self::Error => Some(Severity::Error),
        }
    }
}

/// One entry of `checks.ignore`: every field it names has to match (globs, `*` any text).
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct IgnorePattern {
    pub rule: Option<String>,
    pub url: Option<String>,
    pub selector: Option<String>,
}

/// The section `checks` of the settings, as the schema part `schema/parts/findings.json`
/// describes it.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ChecksSettings {
    pub parallel: Option<usize>,
    pub keep_runs: Option<usize>,
    /// By qualified rule id; a value that is no level is no override.
    pub rules: BTreeMap<String, serde_json::Value>,
    pub ignore: Vec<IgnorePattern>,
}

impl ChecksSettings {
    /// The section from the effective settings (`settings["checks"]`); a missing or broken one
    /// is the default.
    #[must_use]
    pub fn from_value(section: Option<&serde_json::Value>) -> Self {
        section
            .and_then(|value| match serde_json::from_value(value.clone()) {
                Ok(settings) => Some(settings),
                Err(error) => {
                    tracing::warn!(%error, "the settings section checks is malformed; defaults apply");
                    None
                }
            })
            .unwrap_or_default()
    }

    /// `checks.parallel`, between 1 and [`MAX_PARALLEL`].
    #[must_use]
    pub fn parallel(&self) -> usize {
        self.parallel
            .unwrap_or(DEFAULT_PARALLEL)
            .clamp(1, MAX_PARALLEL)
    }

    /// `checks.keepRuns`, between 1 and [`MAX_KEEP_RUNS`].
    #[must_use]
    pub fn keep_runs(&self) -> usize {
        self.keep_runs
            .unwrap_or(DEFAULT_KEEP_RUNS)
            .clamp(1, MAX_KEEP_RUNS)
    }
}

struct CompiledPattern {
    rule: Option<GlobMatcher>,
    url: Option<GlobMatcher>,
    selector: Option<GlobMatcher>,
}

fn compile(glob: Option<&str>) -> Result<Option<GlobMatcher>, globset::Error> {
    glob.map(|glob| Glob::new(glob).map(|glob| glob.compile_matcher()))
        .transpose()
}

fn compile_pattern(pattern: &IgnorePattern) -> Result<CompiledPattern, globset::Error> {
    Ok(CompiledPattern {
        rule: compile(pattern.rule.as_deref())?,
        url: compile(pattern.url.as_deref())?,
        selector: compile(pattern.selector.as_deref())?,
    })
}

fn matches(matcher: Option<&GlobMatcher>, value: Option<&str>) -> bool {
    matcher.is_none_or(|matcher| value.is_some_and(|value| matcher.is_match(value)))
}

/// The rule overrides and the compiled ignore patterns.
pub struct ChecksPolicy {
    levels: BTreeMap<String, RuleLevel>,
    ignore: Vec<CompiledPattern>,
}

impl ChecksPolicy {
    /// Compiles `settings`; a pattern whose glob does not parse is left out (and logged).
    #[must_use]
    pub fn new(settings: &ChecksSettings) -> Self {
        let levels = settings
            .rules
            .iter()
            .filter_map(|(rule, value)| {
                serde_json::from_value::<RuleLevel>(value.clone())
                    .ok()
                    .map(|level| (rule.clone(), level))
            })
            .collect();
        let ignore = settings
            .ignore
            .iter()
            .filter(|pattern| {
                pattern.rule.is_some() || pattern.url.is_some() || pattern.selector.is_some()
            })
            .filter_map(|pattern| {
                compile_pattern(pattern)
                    .map_err(|error| tracing::warn!(%error, "a checks.ignore pattern is left out"))
                    .ok()
            })
            .collect();
        Self { levels, ignore }
    }

    /// The override of `rule_id`, if any.
    #[must_use]
    pub fn level(&self, rule_id: &str) -> Option<RuleLevel> {
        self.levels.get(rule_id).copied()
    }

    /// Whether `rule_id` runs at all.
    #[must_use]
    pub fn enabled(&self, rule_id: &str) -> bool {
        self.level(rule_id) != Some(RuleLevel::Off)
    }

    /// The severity a finding of `rule_id` reported as `reported` gets; `None` when the rule is
    /// off.
    #[must_use]
    pub fn severity(&self, rule_id: &str, reported: Severity) -> Option<Severity> {
        match self.level(rule_id) {
            Some(level) => level.severity(),
            None => Some(reported),
        }
    }

    /// Whether a pattern of `checks.ignore` drops the finding.
    #[must_use]
    pub fn ignored(&self, rule_id: &str, url: Option<&str>, selector: Option<&str>) -> bool {
        self.ignore.iter().any(|pattern| {
            matches(pattern.rule.as_ref(), Some(rule_id))
                && matches(pattern.url.as_ref(), url)
                && matches(pattern.selector.as_ref(), selector)
        })
    }
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
