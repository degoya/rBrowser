//! The plugin's settings (`settings.schema.json`), read through the host: the profiles - test
//! users whose values replace the generated ones in a valid run - the profile used when a run
//! names none, the invalid strategy, the seed (0: a new one per run) and whether hidden fields are
//! left alone. A value of the wrong shape reads as the default.

use std::collections::BTreeMap;

use rb_plugin_api::PluginHost;
use serde_json::Value;

use crate::invalid::Strategy;

/// The most profiles, values per profile and characters per value taken.
pub const MAX_PROFILES: usize = 50;
pub const MAX_VALUES: usize = 100;
pub const MAX_VALUE: usize = 1000;

/// A test user: values by field kind (`email`), field name or field id.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Profile {
    pub name: String,
    pub values: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Settings {
    pub profiles: Vec<Profile>,
    pub active_profile: String,
    pub strategy: Strategy,
    pub seed: u64,
    pub skip_hidden: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            profiles: Vec::new(),
            active_profile: String::new(),
            strategy: Strategy::Mixed,
            seed: 0,
            skip_hidden: true,
        }
    }
}

fn get(host: &mut dyn PluginHost, key: &str) -> Value {
    host.setting(key)
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null)
}

fn profile(value: &Value) -> Option<Profile> {
    let name = value.get("name")?.as_str()?.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return None;
    }
    let values = value
        .get("values")
        .and_then(Value::as_object)
        .map(|map| {
            map.iter()
                .filter_map(|(key, value)| {
                    let text = value.as_str()?;
                    (!key.is_empty() && key.len() <= 200 && text.chars().count() <= MAX_VALUE)
                        .then(|| (key.clone(), text.to_owned()))
                })
                .take(MAX_VALUES)
                .collect()
        })
        .unwrap_or_default();
    Some(Profile {
        name: name.to_owned(),
        values,
    })
}

impl Settings {
    #[must_use]
    pub fn read(host: &mut dyn PluginHost) -> Self {
        let defaults = Self::default();
        let profiles = get(host, "profiles")
            .as_array()
            .map(|list| list.iter().filter_map(profile).take(MAX_PROFILES).collect())
            .unwrap_or_default();
        Self {
            profiles,
            active_profile: get(host, "activeProfile")
                .as_str()
                .map(|name| name.trim().chars().take(80).collect())
                .unwrap_or_default(),
            strategy: get(host, "invalidStrategy")
                .as_str()
                .and_then(Strategy::parse)
                .unwrap_or(defaults.strategy),
            seed: get(host, "seed")
                .as_u64()
                .filter(|seed| *seed <= u64::from(u32::MAX))
                .unwrap_or(defaults.seed),
            skip_hidden: get(host, "skipHidden")
                .as_bool()
                .unwrap_or(defaults.skip_hidden),
        }
    }

    /// The profile named `name`.
    #[must_use]
    pub fn profile(&self, name: &str) -> Option<&Profile> {
        self.profiles.iter().find(|profile| profile.name == name)
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
