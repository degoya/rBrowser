//! The plugin's state across calls, in `.rbrowser/out/network-panel/state.json` (written with
//! `out-write`, read back with `project-read`): the session's switches - mocks on or off, the
//! cache off, the "clear site data" counter, the breakpoints on or off, the asks waiting for the
//! host (RB-080-37) - kept only for the session that set them (a new run of the app starts from
//! the settings again), and what is kept across runs: the rules a person switched off and the
//! breakpoints.

use std::collections::BTreeSet;

use rb_plugin_api::{Failure, PluginHost};
use serde_json::{Value, json};

use crate::context::Settings;

/// The state, relative to the plugin's out folder.
pub const STATE_FILE: &str = "state.json";
/// The state, relative to the project.
pub const STATE_PATH: &str = ".rbrowser/out/network-panel/state.json";
/// The most rules switched off that are kept.
const MAX_DISABLED: usize = 1024;

/// The state of the current session.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub session: String,
    /// The session's mocks switch; the setting when none.
    pub mocks_enabled: Option<bool>,
    /// The session's cache switch; the setting when none.
    pub cache_disabled: Option<bool>,
    /// Raised once per "clear site data"; the host clears when it rises.
    pub clear_site_data: u64,
    /// The rule ids (`<file>#<id>`) switched off.
    pub disabled: BTreeSet<String>,
    /// The breakpoints, in the panel's form (`id`, `urlPattern`, `regex`, `method`,
    /// `resourceType`, `stage`, `views`, `enabled`); kept across runs.
    pub breakpoints: Vec<Value>,
    /// The session's breakpoints switch; on when none.
    pub breakpoints_enabled: Option<bool>,
    /// The asks the host has not answered yet, in the document's form.
    pub asks: Vec<Value>,
    /// Numbers the asks and the breakpoints.
    pub seq: u64,
    /// The last ask the host refused, for the panel.
    pub last_error: Option<String>,
}

impl State {
    /// Whether mocks apply now.
    #[must_use]
    pub fn mocks_enabled(&self, settings: &Settings) -> bool {
        self.mocks_enabled.unwrap_or(settings.mocks_enabled)
    }

    /// Whether the cache is off now.
    #[must_use]
    pub fn cache_disabled(&self, settings: &Settings) -> bool {
        self.cache_disabled.unwrap_or(settings.cache_disabled)
    }

    /// Whether the breakpoints hold requests now.
    #[must_use]
    pub fn breakpoints_enabled(&self) -> bool {
        self.breakpoints_enabled.unwrap_or(true)
    }

    /// The next number of the sequence.
    pub fn next_seq(&mut self) -> u64 {
        self.seq += 1;
        self.seq
    }
}

/// The most breakpoints kept.
pub const MAX_BREAKPOINTS: usize = 128;
/// The most asks waiting.
pub const MAX_ASKS: usize = 64;

/// The state of session `session`: what is stored, its switches dropped when another session
/// set them (or `session` is empty: the host runs no rules).
#[must_use]
pub fn read(host: &mut dyn PluginHost, session: &str) -> State {
    let stored = host
        .project_read(STATE_PATH)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .unwrap_or(Value::Null);
    let disabled = stored["disabled"]
        .as_array()
        .map(|ids| {
            ids.iter()
                .filter_map(Value::as_str)
                .take(MAX_DISABLED)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let breakpoints = stored["breakpoints"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter(|entry| entry.is_object())
                .take(MAX_BREAKPOINTS)
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let mut state = State {
        session: session.to_owned(),
        disabled,
        breakpoints,
        seq: stored["seq"].as_u64().unwrap_or(0),
        ..State::default()
    };
    if !session.is_empty() && stored["session"].as_str() == Some(session) {
        state.mocks_enabled = stored["mocksEnabled"].as_bool();
        state.cache_disabled = stored["cacheDisabled"].as_bool();
        state.clear_site_data = stored["clearSiteData"].as_u64().unwrap_or(0);
        state.breakpoints_enabled = stored["breakpointsEnabled"].as_bool();
        state.asks = stored["asks"]
            .as_array()
            .map(|list| list.iter().take(MAX_ASKS).cloned().collect())
            .unwrap_or_default();
        state.last_error = stored["lastError"].as_str().map(str::to_owned);
    }
    state
}

/// Writes `state`.
///
/// # Errors
///
/// The host could not write it (no project open, the out folder refused).
pub fn write(host: &mut dyn PluginHost, state: &State) -> Result<(), Failure> {
    let text = json!({
        "session": state.session,
        "mocksEnabled": state.mocks_enabled,
        "cacheDisabled": state.cache_disabled,
        "clearSiteData": state.clear_site_data,
        "disabled": state.disabled,
        "breakpoints": state.breakpoints,
        "breakpointsEnabled": state.breakpoints_enabled,
        "asks": state.asks,
        "seq": state.seq,
        "lastError": state.last_error,
    });
    host.out_write(STATE_FILE, text.to_string().as_bytes())
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
