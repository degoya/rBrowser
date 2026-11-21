//! The plugin's settings (`settings.schema.json`), read through the host: the profiles - test
//! users whose values replace the generated ones in a valid run - the profile used when a run
//! names none, the invalid strategy, the seed (0: a new one per run), whether hidden fields are
//! left alone, whether `type=hidden` inputs are taken into a preset and filled from one
//! (RB-090-70), how long a fill waits for the page to settle after a control field and how many
//! passes it makes for fields that appear and whether it ticks consent boxes (RB-094-22). A value of the wrong shape or outside its
//! range reads as the default.

use std::collections::BTreeMap;

use rb_plugin_api::PluginHost;
use serde_json::Value;

use crate::invalid::Strategy;

/// The most profiles, values per profile and characters per value taken.
pub const MAX_PROFILES: usize = 50;
pub const MAX_VALUES: usize = 100;
pub const MAX_VALUE: usize = 1000;
/// The longest quiet window a fill waits for, in milliseconds (`settleMilliseconds`).
pub const MAX_SETTLE_MS: u32 = 2000;
/// The most passes a fill makes (`maxPasses`).
pub const MAX_PASSES: u32 = 5;
/// The shortest and the longest time one fill may take in all, in seconds
/// (`fillBudgetSeconds`).
pub const MIN_FILL_BUDGET_SECONDS: u32 = 5;
pub const MAX_FILL_BUDGET_SECONDS: u32 = 300;

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
    /// `captureHiddenInputs`: a preset takes `type=hidden` inputs, and a fill sets them from a
    /// profile (never a generated value).
    pub hidden_inputs: bool,
    /// `settleMilliseconds`: the quiet window (no DOM change) a fill waits for after it set a
    /// control field; 0 waits for nothing.
    pub settle_ms: u32,
    /// `maxPasses`: how often a fill reads and sets the fields while new ones appear.
    pub max_passes: u32,
    /// `fillBudgetSeconds`: the most one fill takes in all, every view and pass together; past
    /// it no further pass and no further view starts (`crate::passes`).
    pub fill_budget_seconds: u32,
    /// `tickConsent`: a valid fill ticks the consent checkboxes (privacy, terms).
    pub tick_consent: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            profiles: Vec::new(),
            active_profile: String::new(),
            strategy: Strategy::Mixed,
            seed: 0,
            skip_hidden: true,
            hidden_inputs: false,
            settle_ms: 300,
            max_passes: 3,
            fill_budget_seconds: 30,
            tick_consent: true,
        }
    }
}

fn get(host: &mut dyn PluginHost, key: &str) -> Value {
    host.setting(key)
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null)
}

/// The whole number `key` holds within `min..=max`, else `default`.
fn within(host: &mut dyn PluginHost, key: &str, min: u32, max: u32, default: u32) -> u32 {
    get(host, key)
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| (min..=max).contains(n))
        .unwrap_or(default)
}

/// A profile of the settings' or the presets' shape (`{"name", "values"}`), held to the limits;
/// none for a value of another shape.
#[must_use]
pub fn profile(value: &Value) -> Option<Profile> {
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
            hidden_inputs: get(host, "captureHiddenInputs")
                .as_bool()
                .unwrap_or(defaults.hidden_inputs),
            settle_ms: within(
                host,
                "settleMilliseconds",
                0,
                MAX_SETTLE_MS,
                defaults.settle_ms,
            ),
            max_passes: within(host, "maxPasses", 1, MAX_PASSES, defaults.max_passes),
            fill_budget_seconds: within(
                host,
                "fillBudgetSeconds",
                MIN_FILL_BUDGET_SECONDS,
                MAX_FILL_BUDGET_SECONDS,
                defaults.fill_budget_seconds,
            ),
            tick_consent: get(host, "tickConsent")
                .as_bool()
                .unwrap_or(defaults.tick_consent),
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
