//! axe-core's rules as the plugin offers them: the table `runner/dist/rules.json` (written by
//! `runner/scripts/build.mjs` from the pinned axe-core), each rule's usual severity from its
//! impact, the shared ids the seo plugin's rules use for the same problem, and which rules a run
//! selects from the tags, the per-rule switches and the rules the person left on.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use rb_plugin_api::{RuleInfo, Severity};
use serde::Deserialize;

use crate::key;

/// The rule table the build wrote from axe-core.
const RULES_JSON: &str = include_str!("../runner/dist/rules.json");

/// The rules of other plugins that find the same problem (`docs/plugins.md`, "Check plugins"):
/// their findings at the same element of the same page become one. The seo plugin (RB-060-08)
/// uses the first three and `h1-missing`, `heading-order`; the outline plugin (RB-070-03,
/// RB-091-03) the first two, `form-label-missing` and the headings and landmarks after it.
pub const SHARED_IDS: [(&str, &str); 15] = [
    ("image-alt", "img-alt-missing"),
    ("link-name", "link-name-missing"),
    ("html-has-lang", "html-lang-missing"),
    ("label", "form-label-missing"),
    ("page-has-heading-one", "h1-missing"),
    ("heading-order", "heading-order"),
    ("empty-heading", "heading-empty"),
    ("landmark-one-main", "landmark-main-missing"),
    ("landmark-no-duplicate-main", "landmark-main-multiple"),
    ("landmark-no-duplicate-banner", "landmark-banner-multiple"),
    (
        "landmark-no-duplicate-contentinfo",
        "landmark-contentinfo-multiple",
    ),
    ("landmark-banner-is-top-level", "landmark-banner-nested"),
    (
        "landmark-contentinfo-is-top-level",
        "landmark-contentinfo-nested",
    ),
    ("landmark-unique", "landmark-unique"),
    ("region", "content-outside-landmarks"),
];

/// Tags that mark a rule axe itself keeps out of a normal run; such a rule runs only when the
/// tag is asked for or the rule is switched on by its id.
const OPT_IN_TAGS: [&str; 2] = ["experimental", "deprecated"];

/// One rule of axe-core.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AxeRule {
    pub id: String,
    /// `critical`, `serious`, `moderate` or `minor`.
    pub impact: Option<String>,
    pub tags: Vec<String>,
    pub help_url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Table {
    axe_version: String,
    rules: Vec<AxeRule>,
}

static TABLE: LazyLock<Table> = LazyLock::new(|| {
    serde_json::from_str(RULES_JSON).expect("runner/dist/rules.json is checked by its tests")
});

/// The bundled axe-core's version.
#[must_use]
pub fn axe_version() -> &'static str {
    &TABLE.axe_version
}

/// Every rule of the bundled axe-core, by id.
#[must_use]
pub fn all() -> &'static [AxeRule] {
    &TABLE.rules
}

/// The severity of an axe impact: critical and serious are errors, moderate a warning, minor
/// (and none) information.
#[must_use]
pub fn severity_of(impact: Option<&str>) -> Severity {
    match impact {
        Some("critical" | "serious") => Severity::Error,
        Some("moderate") => Severity::Warning,
        _ => Severity::Info,
    }
}

/// The shared id of rule `id`, if another plugin finds the same.
#[must_use]
pub fn shared_id(id: &str) -> Option<&'static str> {
    SHARED_IDS
        .iter()
        .find(|(rule, _)| *rule == id)
        .map(|(_, shared)| *shared)
}

/// The rules as `check.describe` answers them.
#[must_use]
pub fn describe() -> Vec<RuleInfo> {
    all()
        .iter()
        .map(|rule| RuleInfo {
            id: rule.id.clone(),
            title: key("rules", &rule.id),
            severity: severity_of(rule.impact.as_deref()),
            shared_id: shared_id(&rule.id).map(str::to_owned),
            help_url: (!rule.help_url.is_empty()).then(|| rule.help_url.clone()),
        })
        .collect()
}

/// A WCAG conformance tag as (version, level): `wcag2a` is (0, 1), `wcag21aa` (1, 2),
/// `wcag22aaa` (2, 3).
fn wcag_level(tag: &str) -> Option<(u8, usize)> {
    let rest = tag.strip_prefix("wcag2")?;
    let (version, letters) = match rest.as_bytes().first()? {
        b'1' => (1, &rest[1..]),
        b'2' => (2, &rest[1..]),
        b'a' => (0, rest),
        _ => return None,
    };
    let level = letters.len();
    ((1..=3).contains(&level) && letters.bytes().all(|b| b == b'a')).then_some((version, level))
}

/// The axe tags a list of configured tags stands for: a WCAG level includes every lower level
/// and version (`wcag22aa`: `wcag2a`, `wcag2aa`, `wcag21a`, `wcag21aa`, `wcag22a`, `wcag22aa`);
/// every other tag stands for itself.
#[must_use]
pub fn expand_tags(tags: &[String]) -> BTreeSet<String> {
    let mut expanded = BTreeSet::new();
    for tag in tags {
        expanded.insert(tag.clone());
        if let Some((version, level)) = wcag_level(tag) {
            for v in 0..=version {
                for l in 1..=level {
                    let prefix = if v == 0 {
                        "wcag2".to_owned()
                    } else {
                        format!("wcag2{v}")
                    };
                    expanded.insert(format!("{prefix}{}", "a".repeat(l)));
                }
            }
        }
    }
    expanded
}

/// The rule ids a run asks axe for: those the tags select (a rule tagged `experimental` or
/// `deprecated` only when that tag is asked for too), plus those `switches` turns on, minus those
/// it turns off - and of these only the ones in `enabled` (the rules the person left on in
/// `checks.rules`). In the table's order.
#[must_use]
pub fn select(
    tags: &[String],
    switches: &BTreeMap<String, bool>,
    enabled: &[String],
) -> Vec<String> {
    let wanted = expand_tags(tags);
    all()
        .iter()
        .filter(|rule| {
            let tagged = rule.tags.iter().any(|tag| wanted.contains(tag))
                && OPT_IN_TAGS.iter().all(|opt_in| {
                    !rule.tags.iter().any(|tag| tag == opt_in) || wanted.contains(*opt_in)
                });
            switches.get(&rule.id).copied().unwrap_or(tagged)
        })
        .filter(|rule| enabled.iter().any(|id| *id == rule.id))
        .map(|rule| rule.id.clone())
        .collect()
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
