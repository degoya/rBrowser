//! The findings model every check plugin's results share (RB-060-01): what the findings panel,
//! the MCP tool `checks_run`, the CLI `rbrowser check` and `findings.json` show.
//!
//! A plugin answers [`crate::Finding`]s; the host turns them into [`MergedFinding`]s: the rule id
//! qualified with the plugin's id (`seo.images`), a stable [`fingerprint`] as the id, the views
//! and breakpoints it was found in merged, and the findings of different plugins that share a
//! `sharedId` at the same element merged into one with every reporting plugin listed.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{MessageParams, Severity, ViewId};

/// The version of `findings.json` (`schema/findings.v1.json`).
pub const FINDINGS_VERSION: u32 = 1;

/// The schema of `findings.json`, relative to the repository root.
pub const FINDINGS_SCHEMA_FILE: &str = "schema/findings.v1.json";

/// The text of [`FINDINGS_SCHEMA_FILE`]: draft 2020-12, generated from [`FindingsFile`], pretty
/// with a final newline. `rb-plugin-host`'s test `findings_schema` holds the file to it.
#[cfg(feature = "schema")]
#[must_use]
pub fn findings_schema_text() -> String {
    let schema = schemars::generate::SchemaSettings::draft2020_12()
        .into_generator()
        .into_root_schema_for::<FindingsFile>();
    let mut text = serde_json::to_string_pretty(&schema).expect("a schema always serializes");
    text.push('\n');
    text
}

/// What a run checks: every view, or one (`"all"` or a view id, as in every MCP tool).
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum CheckTarget {
    #[default]
    All,
    View(ViewId),
}

impl Serialize for CheckTarget {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::All => serializer.serialize_str("all"),
            Self::View(id) => serializer.serialize_u32(*id),
        }
    }
}

impl<'de> Deserialize<'de> for CheckTarget {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Id(ViewId),
            Text(String),
        }
        match Repr::deserialize(deserializer)? {
            Repr::Id(id) => Ok(Self::View(id)),
            Repr::Text(text) if text == "all" => Ok(Self::All),
            Repr::Text(_) => Err(serde::de::Error::custom(
                "target must be \"all\" or a view id",
            )),
        }
    }
}

#[cfg(feature = "schema")]
impl schemars::JsonSchema for CheckTarget {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "CheckTarget".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "description": "\"all\" or a view id.",
            "anyOf": [
                { "type": "string", "const": "all" },
                { "type": "integer", "minimum": 0 }
            ]
        })
    }
}

/// A finding's text: an i18n key of the reporting plugin's locales and its parameters - never a
/// finished sentence.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct FindingMessage {
    pub key: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, String>,
}

/// The element a finding is about.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ElementRef {
    /// A CSS selector that finds it; what the highlight uses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    /// Its normalised path from the root ([`normalize_path`]); what the id is made of.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// One finding after the run: possibly found by several plugins, in several views.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct MergedFinding {
    /// [`fingerprint`] of the rule (or the shared id), the URL and the element: the same in
    /// every run while the page does not change.
    pub id: String,
    /// `<plugin>.<rule>` of the first reporting plugin: the key of `checks.rules`.
    pub rule_id: String,
    /// Every plugin that reported it, the first one's message kept; more than one only through a
    /// shared id.
    pub plugins: Vec<String>,
    /// The highest of the reports, after `checks.rules`.
    pub severity: Severity,
    pub message: FindingMessage,
    /// The views it was found in, ascending.
    pub views: Vec<ViewId>,
    /// The breakpoint ids of those views, in the same order, each once.
    pub breakpoints: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element: Option<ElementRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help_url: Option<String>,
    /// Free details of the first report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    /// The id the rule shares with other plugins' rules (`img-alt-missing`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared_id: Option<String>,
}

/// How many findings a run has, per severity and per plugin.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct FindingsSummary {
    pub total: usize,
    pub error: usize,
    pub warning: usize,
    pub info: usize,
    /// Per plugin: the findings it reported (a shared finding counts for each).
    pub by_plugin: BTreeMap<String, usize>,
}

impl FindingsSummary {
    /// The counts of `findings`.
    #[must_use]
    pub fn of(findings: &[MergedFinding]) -> Self {
        let mut summary = Self {
            total: findings.len(),
            ..Self::default()
        };
        for finding in findings {
            match finding.severity {
                Severity::Error => summary.error += 1,
                Severity::Warning => summary.warning += 1,
                Severity::Info => summary.info += 1,
            }
            for plugin in &finding.plugins {
                *summary.by_plugin.entry(plugin.clone()).or_default() += 1;
            }
        }
        summary
    }
}

/// One view a run checked: what "only in breakpoint X" is measured against.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CheckedView {
    pub id: ViewId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub breakpoint: Option<String>,
}

/// A plugin that failed in a run: the run goes on without its findings.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct CheckFailure {
    pub plugin: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<ViewId>,
    /// The failure's i18n key (`plugins.error.timed_out`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// The key's parameters (`{service}` of `plugins.error.not_available`, RB-080-98).
    #[serde(default, skip_serializing_if = "MessageParams::is_empty")]
    pub params: MessageParams,
}

/// The reason of a [`CheckSkip`] whose view's engine the plugin cannot serve.
pub const SKIP_ENGINE: &str = "engine";

/// A plugin that did not check a view, and why (RB-080-98): not a failure, and not clean - its
/// findings in that view are unknown.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct CheckSkip {
    pub plugin: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<ViewId>,
    /// Why: [`SKIP_ENGINE`] - the view's engine cannot serve the plugin.
    pub reason: String,
    /// The view's engine (`webkit`) of an engine skip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
}

/// `.rbrowser/out/checks/<run-id>/findings.json`, version [`FINDINGS_VERSION`].
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct FindingsFile {
    pub version: u32,
    /// Sortable by time (`run-<unix milliseconds>`).
    pub run_id: String,
    /// Unix seconds.
    pub started_at: u64,
    pub target: CheckTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The plugins that ran.
    pub plugins: Vec<String>,
    /// The views checked; empty for a run without views.
    #[serde(default)]
    pub views: Vec<CheckedView>,
    pub findings: Vec<MergedFinding>,
    pub summary: FindingsSummary,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<CheckFailure>,
    /// The plugin x view pairs not checked (a view of an engine the plugin cannot serve): their
    /// findings are unknown, never clean (RB-080-98).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<CheckSkip>,
}

/// From which severity on `rbrowser check` exits with 1 (`--fail-on`).
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailOn {
    Info,
    Warning,
    #[default]
    Error,
}

impl FailOn {
    /// Whether `summary` has a finding at or above the threshold.
    #[must_use]
    pub fn fails(self, summary: &FindingsSummary) -> bool {
        match self {
            Self::Error => summary.error > 0,
            Self::Warning => summary.error + summary.warning > 0,
            Self::Info => summary.total > 0,
        }
    }
}

/// The stable id of a finding: FNV-1a (64 bit, hex) over `key` (the rule id or the shared id),
/// the [`normalize_url`]ed URL and the [`normalize_path`]ed element. Not a secret and not a
/// security boundary - only the same in every run, on every platform.
#[must_use]
pub fn fingerprint(key: &str, url: Option<&str>, element: Option<&str>) -> String {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let url = url.map(normalize_url).unwrap_or_default();
    let element = element.map(normalize_path).unwrap_or_default();
    let mut hash = OFFSET;
    for byte in [key, &url, &element].join("\u{1f}").bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    format!("{hash:016x}")
}

fn is_combinator(c: char) -> bool {
    matches!(c, '>' | '+' | '~')
}

/// An element path with whitespace collapsed and none around the combinators `>`, `+`, `~`:
/// `html > body  main` becomes `html>body main`.
#[must_use]
pub fn normalize_path(path: &str) -> String {
    let collapsed: Vec<char> = path
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .collect();
    let mut out = String::with_capacity(collapsed.len());
    for (index, c) in collapsed.iter().enumerate() {
        if *c == ' ' {
            let after_combinator = out.chars().last().is_some_and(is_combinator);
            let before_combinator = collapsed
                .get(index + 1)
                .is_some_and(|next| is_combinator(*next));
            if after_combinator || before_combinator {
                continue;
            }
        }
        out.push(*c);
    }
    out
}

/// A URL without its fragment and outer whitespace.
#[must_use]
pub fn normalize_url(url: &str) -> String {
    let url = url.trim();
    url.split_once('#').map_or(url, |(page, _)| page).to_owned()
}

#[cfg(test)]
#[path = "finding_tests.rs"]
mod tests;
