//! `emulation` in the user settings (RB-080-10, Settings > Emulation > Geolocation, time zone
//! and CPU): the values the shortcuts switch a view to, and the user's own geolocation presets
//! beside the built-in ones (Berlin, New York, Tokyo, Sydney). A built-in preset is named by
//! the i18n key `emulation.geolocation.preset.<id>`. The page clock and the animation rates
//! (RB-094-07, Settings > Emulation > Clock and animations) are `crate::clock_settings`'s.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::clock_settings::{
    ClockUserSettings, DEFAULT_ANIMATION_STEP_MS, default_animation_rates,
};
use crate::{Geolocation, PseudoLocaleSettings};

/// The built-in geolocation presets: id, latitude, longitude.
const BUILTIN_PRESETS: [(&str, f64, f64); 4] = [
    ("berlin", 52.52, 13.405),
    ("new-york", 40.7128, -74.006),
    ("tokyo", 35.6762, 139.6503),
    ("sydney", -33.8688, 151.2093),
];

/// The preset `view.emulate.geolocation` sets by default.
pub const DEFAULT_GEOLOCATION_PRESET: &str = "berlin";
/// The time zone `view.emulate.timezone` sets by default.
pub const DEFAULT_TIMEZONE: &str = "America/New_York";
/// The factor `view.emulate.cpu` sets by default.
pub const DEFAULT_CPU_THROTTLING: f64 = 4.0;

/// `emulation` (user scope).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct EmulationUserSettings {
    /// The geolocation preset (built in or your own) the shortcut sets on a view without one.
    pub default_geolocation: String,
    /// Your own geolocation presets, offered beside Berlin, New York, Tokyo and Sydney.
    pub geolocation_presets: Vec<GeolocationPreset>,
    /// The IANA time zone the shortcut sets on a view without one.
    #[schemars(extend("x-options" = crate::breakpoint::TIMEZONES))]
    pub default_timezone: String,
    /// The CPU throttling factor the shortcut sets on a view without one.
    #[schemars(range(min = 1.0, max = 100.0))]
    pub default_cpu_throttling: f64,
    /// The pseudo-localisation of the page's texts (RB-092-02).
    pub pseudo_locale: PseudoLocaleSettings,
    /// The page clock the clock shortcut sets in every view (RB-094-07).
    pub clock: ClockUserSettings,
    /// The animation rates the animations shortcut steps through: 0 stops them, 1 is normal.
    #[schemars(extend(
        "uniqueItems" = true,
        "items" = { "type": "number", "enum": [0.0, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0] }
    ))]
    pub animation_rates: Vec<f64>,
    /// How far one step moves stopped animations on, in milliseconds.
    #[schemars(range(min = 1, max = 10000))]
    pub animation_step_ms: u32,
}

impl Default for EmulationUserSettings {
    fn default() -> Self {
        Self {
            default_geolocation: DEFAULT_GEOLOCATION_PRESET.to_owned(),
            geolocation_presets: Vec::new(),
            default_timezone: DEFAULT_TIMEZONE.to_owned(),
            default_cpu_throttling: DEFAULT_CPU_THROTTLING,
            pseudo_locale: PseudoLocaleSettings::default(),
            clock: ClockUserSettings::default(),
            animation_rates: default_animation_rates(),
            animation_step_ms: DEFAULT_ANIMATION_STEP_MS,
        }
    }
}

/// A named position.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GeolocationPreset {
    /// Unique among the presets (`office`); a built-in id is replaced by yours.
    pub id: String,
    /// The name shown; built-in presets use an i18n key instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The position.
    #[serde(flatten)]
    pub position: Geolocation,
}

/// The built-in presets, then the user's own; one of the user's with a built-in id replaces it.
pub fn geolocation_presets(settings: &EmulationUserSettings) -> Vec<GeolocationPreset> {
    let mut presets: Vec<GeolocationPreset> = BUILTIN_PRESETS
        .iter()
        .filter(|(id, ..)| !settings.geolocation_presets.iter().any(|own| own.id == *id))
        .map(|(id, latitude, longitude)| GeolocationPreset {
            id: (*id).to_owned(),
            label: None,
            position: Geolocation {
                latitude: *latitude,
                longitude: *longitude,
                accuracy: crate::DEFAULT_ACCURACY_M,
            },
        })
        .collect();
    presets.extend(settings.geolocation_presets.iter().cloned());
    presets
}

/// The position of preset `id` (built in or the user's).
pub fn geolocation_preset(settings: &EmulationUserSettings, id: &str) -> Option<Geolocation> {
    geolocation_presets(settings)
        .into_iter()
        .find(|preset| preset.id == id)
        .map(|preset| preset.position)
}

#[cfg(test)]
#[path = "emulation_settings_tests.rs"]
mod tests;
