//! The settings and the views' choices.
//!
//! A component lives for one call, so the choice of each view is kept in
//! `.rbrowser/out/simulators/state.json` (the project's output folder, out of git) and read again
//! by every call - the page script of a reloaded view asks for its simulation there. The file
//! names views by id; without `persist` the plugin empties it when it starts, so a new session
//! starts with every view as the `default` setting says.

use std::collections::BTreeMap;

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::catalog::Simulation;

/// The state file, relative to the plugin's out folder (`out.write`).
pub const STATE_FILE: &str = "state.json";
/// The same file, relative to the project (`project.read`).
pub const STATE_PATH: &str = ".rbrowser/out/simulators/state.json";
/// The version of the state file this plugin writes and reads.
pub const STATE_VERSION: u32 = 1;
/// The most views the file keeps.
pub const MAX_VIEWS: usize = 64;
/// The blur of `myopia` at full strength without the setting, in CSS px.
pub const DEFAULT_BLUR_PX: f64 = 2.0;
/// The largest `blurPx`.
pub const MAX_BLUR_PX: f64 = 20.0;

/// One view's simulation.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Choice {
    #[serde(rename = "type")]
    pub kind: Simulation,
    /// The strength (0 to 1); none for the simulation's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<f64>,
}

impl Choice {
    /// Whether the strength is one the plugin accepts.
    #[must_use]
    pub fn valid(&self) -> bool {
        self.severity
            .is_none_or(|severity| (0.0..=1.0).contains(&severity))
    }
}

/// The views' choices; a view not named follows the `default` setting.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub views: BTreeMap<ViewId, Choice>,
}

impl State {
    /// The simulation of `view`: its own choice, else the `default` setting.
    #[must_use]
    pub fn effective(&self, view: ViewId, settings: &Settings) -> Choice {
        self.views.get(&view).copied().unwrap_or(Choice {
            kind: settings.default,
            severity: None,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct StateFile {
    version: u32,
    #[serde(default)]
    views: BTreeMap<ViewId, serde_json::Value>,
}

/// The plugin's settings (`settings.schema.json`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// `default`: the simulation of a view without one of its own.
    pub default: Simulation,
    /// `blurPx`: the blur of `myopia` at full strength.
    pub blur_px: f64,
    /// `preferNative`: Chromium renders the types it knows itself.
    pub prefer_native: bool,
    /// `persist`: the choices outlive a restart.
    pub persist: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default: Simulation::None,
            blur_px: DEFAULT_BLUR_PX,
            prefer_native: true,
            persist: false,
        }
    }
}

/// The setting `key`, parsed and checked; a value that does not fit is logged and left out.
fn setting<T: DeserializeOwned>(
    host: &mut dyn PluginHost,
    key: &str,
    check: impl Fn(&T) -> bool,
) -> Option<T> {
    let text = host.setting(key)?;
    match serde_json::from_str::<T>(&text) {
        Ok(value) if check(&value) => Some(value),
        _ => {
            host.log("warn", &format!("setting {key} left out: not usable"));
            None
        }
    }
}

impl Settings {
    /// The effective settings; one that does not read is its default (logged).
    pub fn read(host: &mut dyn PluginHost) -> Self {
        let defaults = Self::default();
        Self {
            default: setting(host, "default", |_: &Simulation| true).unwrap_or(defaults.default),
            blur_px: setting(host, "blurPx", |px: &f64| (0.0..=MAX_BLUR_PX).contains(px))
                .unwrap_or(defaults.blur_px),
            prefer_native: setting(host, "preferNative", |_: &bool| true)
                .unwrap_or(defaults.prefer_native),
            persist: setting(host, "persist", |_: &bool| true).unwrap_or(defaults.persist),
        }
    }
}

/// What reading the file gave: the state, or none when there is no file to read (no project).
fn read(host: &mut dyn PluginHost) -> Option<State> {
    let bytes = match host.project_read(STATE_PATH) {
        Ok(bytes) => bytes,
        Err(failure) if failure.kind == FailureKind::NotFound => return Some(State::default()),
        Err(_) => return None,
    };
    let Some(file) = serde_json::from_slice::<StateFile>(&bytes)
        .ok()
        .filter(|file| file.version == STATE_VERSION)
    else {
        host.log("warn", "state.json is unusable and left out");
        return Some(State::default());
    };
    let views = file
        .views
        .into_iter()
        .filter_map(|(view, value)| {
            serde_json::from_value::<Choice>(value)
                .ok()
                .filter(Choice::valid)
                .map(|choice| (view, choice))
        })
        .take(MAX_VIEWS)
        .collect();
    Some(State { views })
}

/// The views' choices; empty without a file or a project.
pub fn load(host: &mut dyn PluginHost) -> State {
    read(host).unwrap_or_default()
}

/// Writes the state file.
///
/// # Errors
///
/// The host's: no project, or the write failed.
pub fn save(host: &mut dyn PluginHost, state: &State) -> Result<(), Failure> {
    let file = serde_json::json!({ "version": STATE_VERSION, "views": state.views });
    let bytes = serde_json::to_vec_pretty(&file)
        .map_err(|_| Failure::coded(FailureKind::Other, "plugins.error.invalid"))?;
    host.out_write(STATE_FILE, &bytes)
}

/// Empties a state file that names a view; a failure is logged.
pub fn clear(host: &mut dyn PluginHost) {
    let named = read(host).is_some_and(|state| !state.views.is_empty());
    if named && let Err(failure) = save(host, &State::default()) {
        let code = failure.code.unwrap_or_default();
        host.log("warn", &format!("state.json not emptied: {code}"));
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
