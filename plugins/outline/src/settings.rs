//! The plugin's settings (`settings.schema.json`, Settings > DevTools & Debugging > Outline),
//! read at each call and held to the schema's bounds again: a hand-edited config may hold
//! anything. Rules on/off and their severity are the host's `checks.rules`.

use rb_plugin_api::PluginHost;
use serde::Serialize;
use serde_json::Value;

/// The most custom selectors taken.
pub const MAX_CUSTOM: usize = 20;
/// The longest custom selector, in characters.
pub const MAX_SELECTOR_CHARS: usize = 512;
/// The longest custom label, in characters.
pub const MAX_LABEL_CHARS: usize = 80;
/// The `http` rate of the manifest: the most requests a second the host lets through, and the
/// ceiling of `linkCheck.rps` (the same pace as the seo plugin's crawl).
pub const MAX_RPS: u32 = 4;
/// The bounds of `outline.headingMaxLength`, in characters.
pub const MIN_HEADING_CHARS: usize = 20;
pub const MAX_HEADING_CHARS: usize = 1_000;
/// The kinds of element the rules over the breakpoints compare (`outline.diff.elements`,
/// RB-094-02), in the order the findings list them.
pub const DIFF_ELEMENTS: [&str; 3] = ["interactive", "headings", "landmarks"];
/// The largest `outline.readingOrder.tolerancePx`, in CSS px.
pub const MAX_TOLERANCE_PX: i64 = 50;

/// The outline section a kind of [`DIFF_ELEMENTS`] is read from: the interactive elements are
/// the focus list's.
#[must_use]
pub fn diff_section(element: &str) -> Option<&'static str> {
    match element {
        "interactive" => Some("focus"),
        "headings" => Some("headings"),
        "landmarks" => Some("landmarks"),
        _ => None,
    }
}

/// One entry of `outline.customSelectors`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CustomSelector {
    pub label: String,
    pub selector: String,
}

/// The effective settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub custom: Vec<CustomSelector>,
    pub link_check: bool,
    /// Requests a second, 1 to [`MAX_RPS`].
    pub rps: u32,
    /// The time budget of one link check, in milliseconds.
    pub timeout_ms: u64,
    /// A heading longer than this many characters is found (`heading-too-long`).
    pub heading_max_chars: usize,
    /// The kinds of element compared across the breakpoints, among [`DIFF_ELEMENTS`].
    pub diff_elements: Vec<String>,
    /// Boxes overlapping by up to this many CSS px still count as one before the other in the
    /// reading order, 0 to [`MAX_TOLERANCE_PX`].
    pub tolerance_px: i64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            custom: Vec::new(),
            link_check: true,
            rps: 2,
            timeout_ms: 30_000,
            heading_max_chars: 120,
            diff_elements: DIFF_ELEMENTS.map(str::to_owned).to_vec(),
            tolerance_px: 4,
        }
    }
}

fn read(host: &mut dyn PluginHost, key: &str) -> Option<Value> {
    host.setting(key)
        .and_then(|text| serde_json::from_str(&text).ok())
}

/// The custom selectors of `value`: entries with a label and a selector without control
/// characters, cut to their limits, at most [`MAX_CUSTOM`].
#[must_use]
pub fn custom_selectors(value: &Value) -> Vec<CustomSelector> {
    let clean = |value: &Value, max: usize| -> Option<String> {
        let text = value.as_str()?.trim();
        (!text.is_empty() && text.chars().count() <= max && !text.chars().any(char::is_control))
            .then(|| text.to_owned())
    };
    value
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| {
                    Some(CustomSelector {
                        label: clean(&entry["label"], MAX_LABEL_CHARS)?,
                        selector: clean(&entry["selector"], MAX_SELECTOR_CHARS)?,
                    })
                })
                .take(MAX_CUSTOM)
                .collect()
        })
        .unwrap_or_default()
}

/// The kinds of `value` among [`DIFF_ELEMENTS`], each once, in their order; `None` when it is no
/// list (an empty list compares nothing).
fn diff_elements(value: &Value) -> Option<Vec<String>> {
    let list = value.as_array()?;
    Some(
        DIFF_ELEMENTS
            .iter()
            .filter(|kind| list.iter().any(|entry| entry.as_str() == Some(**kind)))
            .map(|kind| (*kind).to_owned())
            .collect(),
    )
}

impl Settings {
    /// The sections the rules over the breakpoints compare: those of the chosen kinds, among
    /// `within` (every section when empty); empty when nothing is to be compared.
    #[must_use]
    pub fn diff_sections(&self, within: &[String]) -> Vec<String> {
        self.diff_elements
            .iter()
            .filter_map(|kind| diff_section(kind.as_str()))
            .filter(|section| {
                within.is_empty() || within.iter().any(|name| name.as_str() == *section)
            })
            .map(str::to_owned)
            .collect()
    }

    /// The settings as the host holds them; a missing or malformed key keeps its default.
    pub fn read(host: &mut dyn PluginHost) -> Self {
        let defaults = Self::default();
        Self {
            custom: read(host, "customSelectors")
                .map(|value| custom_selectors(&value))
                .unwrap_or_default(),
            link_check: read(host, "linkCheck.enabled")
                .and_then(|value| value.as_bool())
                .unwrap_or(defaults.link_check),
            rps: read(host, "linkCheck.rps")
                .and_then(|value| value.as_u64())
                .and_then(|value| u32::try_from(value).ok())
                .map_or(defaults.rps, |value| value.clamp(1, MAX_RPS)),
            timeout_ms: read(host, "linkCheck.timeoutMs")
                .and_then(|value| value.as_u64())
                .map_or(defaults.timeout_ms, |value| value.clamp(1_000, 120_000)),
            heading_max_chars: read(host, "headingMaxLength")
                .and_then(|value| value.as_u64())
                .and_then(|value| usize::try_from(value).ok())
                .map_or(defaults.heading_max_chars, |value| {
                    value.clamp(MIN_HEADING_CHARS, MAX_HEADING_CHARS)
                }),
            diff_elements: read(host, "diff.elements")
                .and_then(|value| diff_elements(&value))
                .unwrap_or(defaults.diff_elements),
            tolerance_px: read(host, "readingOrder.tolerancePx")
                .and_then(|value| value.as_i64())
                .filter(|value| *value >= 0)
                .map_or(defaults.tolerance_px, |value| value.min(MAX_TOLERANCE_PX)),
        }
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
