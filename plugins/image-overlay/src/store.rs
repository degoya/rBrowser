//! Where the state comes from and goes: the shared layer from the plugin's settings, the person's
//! overrides from and to `.rbrowser/out/image-overlay/state.json`.
//!
//! The overrides file lies in the project's output folder, which `rbrowser init` keeps out of git
//! (`.rbrowser/.gitignore`): personal, kept across restarts. A component lives for one call, so
//! every call reads the file again; without a project the page's copy of the overrides stands
//! in, so the views stay in step for the session.
//!
//! A repository can still commit the file - the design on at full opacity over every page - so
//! it is read and written only in a trusted workspace (Charge 8 review L1, as storage-panel's
//! pins): without trust the page's copy stands in, as without a project.

use std::collections::BTreeMap;

use rb_plugin_api::{Failure, FailureKind, PluginHost, codes};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::path::breakpoint_id_ok;
use crate::state::{Anchor, Blend, MAX_DESIGNS, Overrides, Shared};

/// The overrides file, relative to the plugin's out folder (`out.write`).
pub const STATE_FILE: &str = "state.json";
/// The same file, relative to the project (`project.read`).
pub const STATE_PATH: &str = ".rbrowser/out/image-overlay/state.json";
/// The version of the overrides file this plugin writes and reads.
pub const STATE_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StateFile {
    version: u32,
    overrides: Overrides,
}

/// A folder workspace trust protects: the host refuses to list it while the workspace is not
/// trusted, before it looks it up (`docs/plugins.md`, `project.list`).
pub const TRUST_PROBE: &str = ".rbrowser/flows";

/// Whether the open workspace is trusted, as the host's answer for [`TRUST_PROBE`] tells it: a
/// listing or `not_found` is trust; every other answer (untrusted, no project) is not.
pub fn workspace_trusted(host: &mut dyn PluginHost) -> bool {
    match host.project_list(TRUST_PROBE) {
        Ok(_) => true,
        Err(failure) => failure.code.as_deref() == Some(codes::NOT_FOUND),
    }
}

/// What reading the overrides gave.
#[derive(Clone, Debug, PartialEq)]
pub enum Loaded {
    /// The file's overrides - none when there is no file yet or it is unusable.
    Stored(Overrides),
    /// There is no file to read: no project, an untrusted workspace, or the capability is not
    /// served.
    Unavailable,
}

/// The setting `key`, parsed and checked; a value that does not fit is logged and left out.
fn setting<T: DeserializeOwned>(
    host: &mut dyn PluginHost,
    key: &str,
    fits: impl Fn(&T) -> bool,
) -> Option<T> {
    let text = host.setting(key)?;
    match serde_json::from_str::<T>(&text) {
        Ok(value) if fits(&value) => Some(value),
        _ => {
            host.log("warn", &format!("setting {key} left out: it does not fit"));
            None
        }
    }
}

/// The shared layer: `opacity`, `blend`, `invert`, `anchor`, `autoScaleDpr`, `designs`. A design
/// path is kept as it is: the view it names checks it and reports a refusal.
pub fn shared(host: &mut dyn PluginHost) -> Shared {
    let default = Shared::default();
    Shared {
        opacity: setting(host, "opacity", |value: &u8| *value <= 100).unwrap_or(default.opacity),
        blend: setting(host, "blend", |_: &Blend| true).unwrap_or(default.blend),
        invert: setting(host, "invert", |_: &bool| true).unwrap_or(default.invert),
        anchor: setting(host, "anchor", |_: &Anchor| true).unwrap_or(default.anchor),
        auto_scale_dpr: setting(host, "autoScaleDpr", |_: &bool| true)
            .unwrap_or(default.auto_scale_dpr),
        designs: setting(host, "designs", |map: &BTreeMap<String, String>| {
            map.len() <= MAX_DESIGNS && map.keys().all(|id| breakpoint_id_ok(id))
        })
        .unwrap_or(default.designs),
    }
}

/// Reads the overrides file; [`Loaded::Unavailable`] in an untrusted workspace.
pub fn load(host: &mut dyn PluginHost) -> Loaded {
    if !workspace_trusted(host) {
        return Loaded::Unavailable;
    }
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
/// `plugins.error.untrusted_workspace` (the file is left as it is); the host's: no project, or
/// the write failed.
pub fn save(host: &mut dyn PluginHost, overrides: &Overrides) -> Result<(), Failure> {
    if !workspace_trusted(host) {
        return Err(Failure::coded(
            FailureKind::AccessDenied,
            codes::UNTRUSTED_WORKSPACE,
        ));
    }
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
