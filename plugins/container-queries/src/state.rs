//! Whether the overlay shows, in every view and per view.
//!
//! A component lives for one call, so the state lives in the project:
//! `.rbrowser/out/container-queries/state.json`, written with `out-write` and read back with
//! `project-read`. `all` is what every view shows; `views` what one view shows instead (an MCP
//! call with a view as `target`). View ids do not outlive a start, so `activate` forgets `views`.

use std::collections::BTreeMap;

use rb_plugin_api::{Failure, PluginHost, ViewId};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The file below the plugin's out folder (`out.write`).
pub const STATE_FILE: &str = "state.json";
/// The same file from the project root (`project.read`).
pub const STATE_PATH: &str = ".rbrowser/out/container-queries/state.json";
/// The format of the state file.
pub const STATE_VERSION: u32 = 1;

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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub version: u32,
    /// Whether every view shows the overlay.
    pub all: bool,
    /// What one view shows instead, by view id (as text).
    pub views: BTreeMap<String, bool>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            all: false,
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
            Ok(mut state) if state.version == STATE_VERSION => {
                state.views.retain(|view, _| view.parse::<ViewId>().is_ok());
                state
            }
            _ => {
                host.log(
                    "warn",
                    "container-queries: state.json is unreadable; the overlay starts off",
                );
                Self::default()
            }
        }
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

    /// Whether `view` (every view for none) shows the overlay.
    #[must_use]
    pub fn effective(&self, view: Option<ViewId>) -> bool {
        view.and_then(|view| self.views.get(&view.to_string()).copied())
            .unwrap_or(self.all)
    }

    /// Shows or hides the overlay for `target`: for every view (and no view keeps its own), or
    /// for one.
    pub fn set(&mut self, target: Target, on: bool) {
        match target {
            Target::All => {
                self.views.clear();
                self.all = on;
            }
            Target::View(view) => {
                self.views.insert(view.to_string(), on);
            }
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
