//! Where the state comes from and goes: the shared layer from the plugin's settings, the person's
//! overrides from and to `.rbrowser/out/rulers-guides/state.json`.
//!
//! The overrides file lies in the project's output folder, which `rbrowser init` keeps out of git
//! (`.rbrowser/.gitignore`): personal, kept across restarts. What the team shares is set as the
//! plugin's settings at project scope (`config.json`). A component lives for one call, so every
//! call reads the file again; without a project (no file to read or write) the page's copy of
//! the overrides stands in, so the views stay in step for the session.

use std::collections::BTreeMap;

use rb_plugin_api::{Failure, FailureKind, PluginHost};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::colors::{Colors, is_color};
use crate::model::{
    ANY_BREAKPOINT, Baseline, ColumnSpec, Guide, GuideUnit, Invalid, Overrides, Preset, Shared,
    Visible, check_columns, check_guides,
};

/// The overrides file, relative to the plugin's out folder (`out.write`).
pub const STATE_FILE: &str = "state.json";
/// The same file, relative to the project (`project.read`).
pub const STATE_PATH: &str = ".rbrowser/out/rulers-guides/state.json";
/// The version of the overrides file this plugin writes and reads.
pub const STATE_VERSION: u32 = 1;
/// The snapping step of a `Shift` drag without the setting.
pub const DEFAULT_SNAP_PX: u32 = 8;
/// The largest snapping step.
pub const MAX_SNAP_PX: u32 = 64;

/// The person's preferences that are no part of the drawn state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prefs {
    /// The unit of a vertical guide dragged out of the ruler.
    pub unit: GuideUnit,
    /// The step a `Shift` drag snaps to.
    pub snap_px: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StateFile {
    version: u32,
    overrides: Overrides,
}

/// What reading the overrides gave.
#[derive(Clone, Debug, PartialEq)]
pub enum Loaded {
    /// The file's overrides - none when there is no file yet or it is unusable.
    Stored(Overrides),
    /// There is no file to read: no project, or the capability is not served.
    Unavailable,
}

/// The setting `key`, parsed and checked; a value that does not fit is logged and left out.
fn setting<T: DeserializeOwned>(
    host: &mut dyn PluginHost,
    key: &str,
    check: impl Fn(&T) -> Result<(), Invalid>,
) -> Option<T> {
    let text = host.setting(key)?;
    match serde_json::from_str::<T>(&text) {
        Ok(value) => match check(&value) {
            Ok(()) => Some(value),
            Err(field) => {
                host.log(
                    "warn",
                    &format!("setting {key} left out: {field} out of range"),
                );
                None
            }
        },
        Err(_) => {
            host.log("warn", &format!("setting {key} left out: not readable"));
            None
        }
    }
}

/// The schema's default of `columns`: Bootstrap's grid for every breakpoint.
#[must_use]
pub fn default_columns() -> BTreeMap<String, ColumnSpec> {
    BTreeMap::from([(
        ANY_BREAKPOINT.to_owned(),
        ColumnSpec {
            preset: Some(Preset::Bootstrap),
            ..ColumnSpec::default()
        },
    )])
}

/// The shared layer: `visible`, `guides`, `columns`, `baseline.lineHeight`, `baseline.offset`,
/// `colors.*`.
pub fn shared(host: &mut dyn PluginHost) -> Shared {
    Shared {
        visible: Visible {
            rulers: setting::<bool>(host, "visible", |_| Ok(())).unwrap_or(false),
            columns: false,
            baseline: false,
        },
        guides: setting(host, "guides", |guides: &Vec<Guide>| check_guides(guides))
            .unwrap_or_default(),
        columns: setting(host, "columns", check_columns).unwrap_or_else(default_columns),
        baseline: baseline(host),
        colors: colors(host),
    }
}

/// The colours of the settings `colors.*`; one that is no colour keeps its default.
fn colors(host: &mut dyn PluginHost) -> Colors {
    let mut colors = Colors::default();
    for (key, field) in [
        ("colors.ruler", &mut colors.ruler),
        ("colors.ticks", &mut colors.ticks),
        ("colors.guide", &mut colors.guide),
        ("colors.columnFill", &mut colors.column_fill),
        ("colors.columnEdge", &mut colors.column_edge),
        ("colors.baseline", &mut colors.baseline),
    ] {
        let check = |text: &String| if is_color(text) { Ok(()) } else { Err("color") };
        if let Some(value) = setting(host, key, check) {
            *field = value;
        }
    }
    colors
}

/// The baseline grid of the settings `baseline.lineHeight` and `baseline.offset`; a pair out of
/// range falls back to the default as a whole.
fn baseline(host: &mut dyn PluginHost) -> Baseline {
    let default = Baseline::default();
    let finite = |value: &f64| {
        if value.is_finite() {
            Ok(())
        } else {
            Err("baseline")
        }
    };
    let candidate = Baseline {
        line_height: setting(host, "baseline.lineHeight", finite).unwrap_or(default.line_height),
        offset: setting(host, "baseline.offset", finite).unwrap_or(default.offset),
    };
    match candidate.check() {
        Ok(()) => candidate,
        Err(field) => {
            host.log(
                "warn",
                &format!("setting baseline left out: {field} out of range"),
            );
            default
        }
    }
}

/// The preferences: `unit`, `snapPx`.
pub fn prefs(host: &mut dyn PluginHost) -> Prefs {
    Prefs {
        unit: setting(host, "unit", |_: &GuideUnit| Ok(())).unwrap_or_default(),
        snap_px: setting(host, "snapPx", |step: &u32| {
            if *step <= MAX_SNAP_PX {
                Ok(())
            } else {
                Err("snapPx")
            }
        })
        .unwrap_or(DEFAULT_SNAP_PX),
    }
}

/// Reads the overrides file.
pub fn load(host: &mut dyn PluginHost) -> Loaded {
    match host.project_read(STATE_PATH) {
        Ok(bytes) => {
            let parsed = serde_json::from_slice::<StateFile>(&bytes)
                .ok()
                .filter(|file| file.version == STATE_VERSION)
                .filter(|file| file.overrides.check().is_ok());
            match parsed {
                Some(file) => Loaded::Stored(file.overrides),
                None => {
                    host.log("warn", "state.json is unusable and left out");
                    Loaded::Stored(Overrides::default())
                }
            }
        }
        Err(failure) if failure.kind == FailureKind::NotFound => {
            Loaded::Stored(Overrides::default())
        }
        Err(_) => Loaded::Unavailable,
    }
}

/// The overrides to work on: the file's, or - when it cannot be read - the page's copy.
pub fn overrides(host: &mut dyn PluginHost, page: Option<Overrides>) -> Overrides {
    match load(host) {
        Loaded::Stored(overrides) => overrides,
        Loaded::Unavailable => page
            .filter(|overrides| overrides.check().is_ok())
            .unwrap_or_default(),
    }
}

/// Writes the overrides file.
///
/// # Errors
///
/// The host's: no project, or the write failed.
pub fn save(host: &mut dyn PluginHost, overrides: &Overrides) -> Result<(), Failure> {
    let file = StateFile {
        version: STATE_VERSION,
        overrides: overrides.clone(),
    };
    let bytes = serde_json::to_vec_pretty(&file)
        .map_err(|_| Failure::coded(FailureKind::Other, "plugins.error.invalid"))?;
    host.out_write(STATE_FILE, &bytes)
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
