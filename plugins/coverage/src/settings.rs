//! The plugin's settings (`settings.schema.json`, Settings > Checks > Coverage), read at each call
//! and held to the schema's bounds again: a hand-edited config may hold anything. Rules on/off and
//! their severity are the host's `checks.rules`.

use rb_plugin_api::PluginHost;
/// `scheme://host[:port]` of an `http(s)` URL as the page gives it, the default port dropped:
/// the app's own (RB-094-10).
pub use rb_util::url::origin_of as origin;
use serde_json::Value;

use crate::model::Mode;

/// The most ignore patterns taken.
pub const MAX_PATTERNS: usize = 64;
/// The longest ignore pattern.
const MAX_PATTERN_CHARS: usize = 512;

/// The effective settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// A file smaller than this (UTF-16 units, DevTools' bytes) gets no unused-file warning.
    pub min_bytes: u64,
    /// A file with more unused than this share, in percent, in every view is reported.
    pub unused_percent: u64,
    /// The mode `coverage_get` measures without one.
    pub default_mode: Mode,
    /// Whether files from other origins than the page's are measured.
    pub third_party: bool,
    /// URL globs (`*` any characters) of files left out.
    pub ignore: Vec<String>,
    /// How long no new request must come before a loaded page counts as idle.
    pub idle_ms: u64,
    /// How long a view may take to load again.
    pub load_timeout_seconds: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            min_bytes: 10_240,
            unused_percent: 50,
            default_mode: Mode::Load,
            third_party: true,
            ignore: Vec::new(),
            idle_ms: 500,
            load_timeout_seconds: 30,
        }
    }
}

fn read(host: &mut dyn PluginHost, key: &str) -> Option<Value> {
    host.setting(key)
        .and_then(|text| serde_json::from_str(&text).ok())
}

fn integer(value: Option<Value>, min: u64, max: u64, default: u64) -> u64 {
    value
        .and_then(|value| value.as_u64())
        .map_or(default, |value| value.clamp(min, max))
}

/// The ignore patterns of `value`: non-empty strings, at most [`MAX_PATTERNS`].
#[must_use]
pub fn patterns(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty() && text.chars().count() <= MAX_PATTERN_CHARS)
                .map(str::to_owned)
                .take(MAX_PATTERNS)
                .collect()
        })
        .unwrap_or_default()
}

/// Whether `text` matches `pattern`: `*` stands for any characters, everything else for itself.
#[must_use]
pub fn glob(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    let (Some(first), Some(last)) = (parts.first(), parts.last()) else {
        return false;
    };
    if parts.len() == 1 {
        return pattern == text;
    }
    if !text.starts_with(first) || !text[first.len()..].ends_with(last) {
        return false;
    }
    let mut rest = &text[first.len()..text.len() - last.len()];
    for part in &parts[1..parts.len() - 1] {
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }
    text.len() >= first.len() + last.len()
}

impl Settings {
    /// The settings as the host holds them; a missing or malformed key keeps its default.
    pub fn read(host: &mut dyn PluginHost) -> Self {
        let d = Self::default();
        Self {
            min_bytes: integer(read(host, "minBytes"), 0, 100 * 1024 * 1024, d.min_bytes),
            unused_percent: integer(read(host, "unusedPercent"), 0, 100, d.unused_percent),
            default_mode: match read(host, "defaultMode").as_ref().and_then(Value::as_str) {
                Some("interaction") => Mode::Interaction,
                _ => d.default_mode,
            },
            third_party: read(host, "thirdParty")
                .and_then(|value| value.as_bool())
                .unwrap_or(d.third_party),
            ignore: read(host, "ignore")
                .map(|value| patterns(&value))
                .unwrap_or_default(),
            idle_ms: integer(read(host, "idleMs"), 100, 10_000, d.idle_ms),
            load_timeout_seconds: integer(
                read(host, "loadTimeoutSeconds"),
                5,
                120,
                d.load_timeout_seconds,
            ),
        }
    }

    /// Whether a file at `url` is left out on a page at `page`: an ignore pattern matches it, or
    /// it comes from another origin while third-party files are off.
    #[must_use]
    pub fn skips(&self, url: &str, page: &str) -> bool {
        if self.ignore.iter().any(|pattern| glob(pattern, url)) {
            return true;
        }
        !self.third_party && origin(url) != origin(page)
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
