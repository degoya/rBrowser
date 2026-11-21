//! The plugin's settings (`settings.schema.json`, Settings > Checks > Consent check), read at
//! each call and held to the schema's bounds again: a hand-edited config may hold anything.
//! Rules on/off and their severity are the host's `checks.rules`.

use rb_plugin_api::PluginHost;
use serde_json::Value;

use crate::site::{HostRule, host_rules};

/// The most essential cookie names taken.
pub const MAX_COOKIE_NAMES: usize = 64;
/// The longest essential cookie name, in characters.
const MAX_NAME_CHARS: usize = 256;

/// The effective settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// `allowHosts`: hosts counted as the page's own (its CDNs): no finding.
    pub allow_hosts: Vec<HostRule>,
    /// `rejectStep`: the probe session clicks "Reject all" and captures again.
    pub reject_step: bool,
    /// `essentialCookies`: the project's own necessary cookies (a trailing `*` is a prefix).
    pub essential_cookies: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            allow_hosts: Vec::new(),
            reject_step: true,
            essential_cookies: Vec::new(),
        }
    }
}

fn read(host: &mut dyn PluginHost, key: &str) -> Option<Value> {
    host.setting(key)
        .and_then(|text| serde_json::from_str(&text).ok())
}

fn strings(value: Option<Value>) -> Vec<String> {
    value
        .as_ref()
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// A cookie name of RFC 6265's token characters, with an optional trailing `*`.
fn cookie_name(name: &str) -> bool {
    let body = name.strip_suffix('*').unwrap_or(name);
    !body.is_empty()
        && name.chars().count() <= MAX_NAME_CHARS
        && body
            .chars()
            .all(|c| c.is_ascii_graphic() && !"()<>@,;:\\\"/[]?={}".contains(c))
}

impl Settings {
    /// The settings as the host holds them.
    pub fn read(host: &mut dyn PluginHost) -> Self {
        let defaults = Self::default();
        Self {
            allow_hosts: host_rules(&strings(read(host, "allowHosts"))),
            reject_step: read(host, "rejectStep")
                .and_then(|value| value.as_bool())
                .unwrap_or(defaults.reject_step),
            essential_cookies: strings(read(host, "essentialCookies"))
                .into_iter()
                .filter(|name| cookie_name(name))
                .take(MAX_COOKIE_NAMES)
                .collect(),
        }
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
