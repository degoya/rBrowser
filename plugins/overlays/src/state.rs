//! The overlay state: which overlay is on, with which options, for every view and per view.
//!
//! A component lives for one call, so the state lives in the project:
//! `.rbrowser/out/overlays/state.json`, written with `out-write` and read back with
//! `project-read`. `all` holds what every view shows; `views` what one view shows instead (an MCP
//! call with a view as `target`). View ids do not outlive a start, so `activate` forgets
//! `views` (and, with `overlays.restoreOnStart` off, everything).

use std::collections::BTreeMap;

use rb_plugin_api::{Failure, PluginHost, ViewId};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The file below the plugin's out folder (`out.write`).
pub const STATE_FILE: &str = "state.json";
/// The same file from the project root (`project.read`).
pub const STATE_PATH: &str = ".rbrowser/out/overlays/state.json";
/// The format of the state file.
pub const STATE_VERSION: u32 = 1;

/// One overlay's state.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OverlayState {
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub options: Map<String, Value>,
}

/// Every view, or one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    All,
    View(ViewId),
}

impl Target {
    /// The `target` of a call's arguments: a view id, or every view for `"all"` or none.
    #[must_use]
    pub fn from_args(args: &Value) -> Self {
        args.get("target")
            .and_then(Value::as_u64)
            .and_then(|view| ViewId::try_from(view).ok())
            .map_or(Self::All, Self::View)
    }

    /// The view, for [`State::effective`].
    #[must_use]
    pub fn view(self) -> Option<ViewId> {
        match self {
            Self::All => None,
            Self::View(view) => Some(view),
        }
    }
}

/// The state file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub version: u32,
    /// What every view shows, by overlay id.
    pub all: BTreeMap<String, OverlayState>,
    /// What one view shows instead, by view id (as text) and overlay id.
    pub views: BTreeMap<String, BTreeMap<String, OverlayState>>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            all: BTreeMap::new(),
            views: BTreeMap::new(),
        }
    }
}

impl State {
    /// The project's state; the default when there is none or it does not parse.
    pub fn load(host: &mut dyn PluginHost) -> Self {
        let Ok(bytes) = host.project_read(STATE_PATH) else {
            return Self::default();
        };
        match serde_json::from_slice::<Self>(&bytes) {
            Ok(state) if state.version == STATE_VERSION => state.checked(),
            _ => {
                host.log(
                    "warn",
                    "overlays: state.json is unreadable; starting with every overlay off",
                );
                Self::default()
            }
        }
    }

    /// The state with what the file may not hold dropped: an unknown overlay, or one whose
    /// options its overlay does not take (the file is the project's, not the plugin's).
    #[must_use]
    pub fn checked(mut self) -> Self {
        let keep = |overlays: &mut BTreeMap<String, OverlayState>| {
            overlays.retain(|id, state| {
                crate::overlay::kind(id).is_some_and(|kind| {
                    crate::overlay::validate_options(kind, &state.options)
                        .is_ok_and(|options| options == state.options)
                })
            });
        };
        keep(&mut self.all);
        for overlays in self.views.values_mut() {
            keep(overlays);
        }
        self.views
            .retain(|view, overlays| !overlays.is_empty() && view.parse::<ViewId>().is_ok());
        self
    }

    /// Writes the state to the project.
    ///
    /// # Errors
    ///
    /// `out.write`'s.
    pub fn save(&self, host: &mut dyn PluginHost) -> Result<(), Failure> {
        let bytes = serde_json::to_vec(self).unwrap_or_default();
        host.out_write(STATE_FILE, &bytes)
    }

    /// What `view` (every view for none) shows of `overlay`.
    #[must_use]
    pub fn effective(&self, view: Option<ViewId>, overlay: &str) -> OverlayState {
        view.and_then(|view| self.views.get(&view.to_string()))
            .and_then(|overlays| overlays.get(overlay))
            .or_else(|| self.all.get(overlay))
            .cloned()
            .unwrap_or_default()
    }

    /// Sets `overlay` for `target`: for every view (and no view keeps its own), or for one.
    pub fn set(&mut self, target: Target, overlay: &str, state: OverlayState) {
        match target {
            Target::All => {
                for overlays in self.views.values_mut() {
                    overlays.remove(overlay);
                }
                self.views.retain(|_, overlays| !overlays.is_empty());
                self.all.insert(overlay.to_owned(), state);
            }
            Target::View(view) => {
                self.views
                    .entry(view.to_string())
                    .or_default()
                    .insert(overlay.to_owned(), state);
            }
        }
    }

    /// Every overlay off in every view, the options kept.
    pub fn off(&mut self) {
        self.views.clear();
        for state in self.all.values_mut() {
            state.enabled = false;
        }
    }

    /// Forgets what single views show (their ids are gone after a start).
    pub fn forget_views(&mut self) {
        self.views.clear();
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
