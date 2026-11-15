//! The plugin's settings (`settings.schema.json`, Settings > Checks > Fonts), read at each call
//! and held to the schema's bounds again: a hand-edited config may hold anything. Rules on/off and
//! their severity are the host's `checks.rules`.

use rb_plugin_api::PluginHost;
use serde_json::Value;

/// The most allowed hosts taken.
pub const MAX_HOSTS: usize = 64;
/// The longest host name (DNS).
const MAX_HOST_CHARS: usize = 253;

/// The effective settings.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// Font hosts that are no privacy finding: `fonts.example.com` or `*.example.com`.
    pub allowed_hosts: Vec<String>,
    /// Text below this size, in CSS px, is reported.
    pub min_text_px: f64,
    /// Form fields below this size are reported on views up to 1024 px wide (iOS zooms in).
    pub min_input_px: f64,
    /// Paragraphs with a smaller line height (relative to the font size) are reported.
    pub min_line_height: f64,
    /// More loaded weights and styles of one family are reported.
    pub max_weights: u32,
    /// Font files larger than this, in KiB, are reported.
    pub max_file_kb: u64,
    /// A heading should grow at least by this factor from the narrowest to the widest view.
    pub fluid_min_ratio: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            allowed_hosts: Vec::new(),
            min_text_px: 12.0,
            min_input_px: 16.0,
            min_line_height: 1.2,
            max_weights: 4,
            max_file_kb: 100,
            fluid_min_ratio: 1.15,
        }
    }
}

fn read(host: &mut dyn PluginHost, key: &str) -> Option<Value> {
    host.setting(key)
        .and_then(|text| serde_json::from_str(&text).ok())
}

fn number(value: Option<Value>, min: f64, max: f64, default: f64) -> f64 {
    value
        .and_then(|value| value.as_f64())
        .filter(|value| value.is_finite())
        .map_or(default, |value| value.clamp(min, max))
}

/// One allowed host: lower case, letters, digits, `-` and `.`, optionally behind `*.`.
fn clean_host(value: &Value) -> Option<String> {
    let text = value.as_str()?.trim().to_ascii_lowercase();
    let name = text.strip_prefix("*.").unwrap_or(&text);
    let ok = !name.is_empty()
        && text.len() <= MAX_HOST_CHARS
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.');
    ok.then_some(text)
}

/// The allowed hosts of `value`, malformed entries dropped, at most [`MAX_HOSTS`].
#[must_use]
pub fn allowed_hosts(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|list| list.iter().filter_map(clean_host).take(MAX_HOSTS).collect())
        .unwrap_or_default()
}

impl Settings {
    /// The settings as the host holds them; a missing or malformed key keeps its default.
    pub fn read(host: &mut dyn PluginHost) -> Self {
        let d = Self::default();
        Self {
            allowed_hosts: read(host, "allowedHosts")
                .map(|value| allowed_hosts(&value))
                .unwrap_or_default(),
            min_text_px: number(read(host, "minTextPx"), 6.0, 32.0, d.min_text_px),
            min_input_px: number(read(host, "minInputPx"), 8.0, 32.0, d.min_input_px),
            min_line_height: number(read(host, "minLineHeight"), 0.8, 3.0, d.min_line_height),
            max_weights: read(host, "maxWeightsPerFamily")
                .and_then(|value| value.as_u64())
                .and_then(|value| u32::try_from(value).ok())
                .map_or(d.max_weights, |value| value.clamp(1, 20)),
            max_file_kb: read(host, "maxFileKb")
                .and_then(|value| value.as_u64())
                .map_or(d.max_file_kb, |value| value.clamp(1, 10_000)),
            fluid_min_ratio: number(read(host, "fluidMinRatio"), 1.0, 4.0, d.fluid_min_ratio),
        }
    }

    /// Whether fonts from `host` are allowed: the name itself, or below a `*.` entry's domain.
    #[must_use]
    pub fn host_allowed(&self, host: &str) -> bool {
        let host = host.to_ascii_lowercase();
        self.allowed_hosts
            .iter()
            .any(|entry| match entry.strip_prefix("*.") {
                Some(domain) => host == domain || host.ends_with(&format!(".{domain}")),
                None => host == *entry,
            })
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
