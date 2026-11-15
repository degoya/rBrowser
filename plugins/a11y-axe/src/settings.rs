//! The plugin's settings (`settings.schema.json`, Settings > Checks > Accessibility): which axe
//! tags run, single rules on or off, the parts of the page to check or leave out, and whether
//! axe's "needs review" results are reported. Read through `settings.get`, held to the schema's
//! bounds again here - a value that does not fit fails the run with the key named.

use std::collections::BTreeMap;

use rb_plugin_api::{Failure, FailureKind, PluginHost};
use serde::de::DeserializeOwned;

use crate::key;

/// The tags a run uses when nothing is set: WCAG 2.2 level AA (with every lower level).
pub const DEFAULT_TAGS: [&str; 1] = ["wcag22aa"];
/// The most tags, selectors or rule switches a setting holds.
pub const MAX_ITEMS: usize = 32;
/// The most rule switches.
pub const MAX_SWITCHES: usize = 256;
/// The longest selector.
pub const MAX_SELECTOR: usize = 256;

/// The effective settings of one run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub tags: Vec<String>,
    pub rules: BTreeMap<String, bool>,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub report_incomplete: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            tags: DEFAULT_TAGS.iter().map(|tag| (*tag).to_owned()).collect(),
            rules: BTreeMap::new(),
            include: Vec::new(),
            exclude: Vec::new(),
            report_incomplete: false,
        }
    }
}

/// `plugin.a11y-axe.errors.bad_setting` for key `name`.
pub fn bad_setting(name: &str, reason: &str) -> Failure {
    Failure::coded(FailureKind::BadData, &key("errors", "bad_setting"))
        .with_param("key", name)
        .with_param("reason", reason)
}

/// Whether `tag` may name an axe tag: 1 to 64 ASCII letters, digits, `.`, `_` and `-`.
#[must_use]
pub fn valid_tag(tag: &str) -> bool {
    (1..=64).contains(&tag.len())
        && tag.starts_with(|c: char| c.is_ascii_alphanumeric())
        && tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Holds a tag list to its bounds.
///
/// # Errors
///
/// More than [`MAX_ITEMS`] tags or a malformed one.
pub fn check_tags(name: &str, tags: &[String]) -> Result<(), Failure> {
    if tags.len() > MAX_ITEMS {
        return Err(bad_setting(name, "too many tags"));
    }
    match tags.iter().find(|tag| !valid_tag(tag)) {
        Some(tag) => Err(bad_setting(name, &format!("`{tag}` is no axe tag"))),
        None => Ok(()),
    }
}

fn check_selectors(name: &str, selectors: &[String]) -> Result<(), Failure> {
    if selectors.len() > MAX_ITEMS {
        return Err(bad_setting(name, "too many selectors"));
    }
    let bad = selectors.iter().find(|selector| {
        selector.trim().is_empty()
            || selector.len() > MAX_SELECTOR
            || selector.chars().any(char::is_control)
    });
    match bad {
        Some(_) => Err(bad_setting(
            name,
            "a selector is empty, too long or holds control characters",
        )),
        None => Ok(()),
    }
}

fn read<T: DeserializeOwned>(
    host: &mut dyn PluginHost,
    name: &str,
    fallback: T,
) -> Result<T, Failure> {
    match host.setting(name) {
        None => Ok(fallback),
        Some(text) if text.trim() == "null" => Ok(fallback),
        Some(text) => {
            serde_json::from_str(&text).map_err(|error| bad_setting(name, &error.to_string()))
        }
    }
}

impl Settings {
    /// The effective settings; the schema's default for a key the host has no value for.
    ///
    /// # Errors
    ///
    /// A value that does not fit its key ([`bad_setting`]).
    pub fn read(host: &mut dyn PluginHost) -> Result<Self, Failure> {
        let defaults = Self::default();
        let settings = Self {
            tags: read(host, "tags", defaults.tags)?,
            rules: read(host, "rules", defaults.rules)?,
            include: read(host, "include", defaults.include)?,
            exclude: read(host, "exclude", defaults.exclude)?,
            report_incomplete: read(host, "reportIncomplete", defaults.report_incomplete)?,
        };
        check_tags("tags", &settings.tags)?;
        if settings.rules.len() > MAX_SWITCHES {
            return Err(bad_setting("rules", "too many rules"));
        }
        if let Some(id) = settings
            .rules
            .keys()
            .find(|id| !rb_plugin_api::check::valid_rule_id(id))
        {
            return Err(bad_setting("rules", &format!("`{id}` is no rule id")));
        }
        check_selectors("include", &settings.include)?;
        check_selectors("exclude", &settings.exclude)?;
        Ok(settings)
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
