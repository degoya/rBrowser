//! The plugin's settings (`settings.schema.json`, Settings > DevTools & Debugging > Console) and
//! the clear mark.
//!
//! A component keeps nothing between calls, so "clear" is a mark on disk: the time of the newest
//! message when the console was cleared, in `.rbrowser/out/console/state.json` (written with
//! `out-write`, read back with `project-read`). Messages at or before it are not shown; one
//! mark serves the panel, the command and MCP alike. A mark is never later than the clock, so a
//! made-up capture cannot hide the messages still to come.

use std::collections::BTreeSet;

use rb_plugin_api::PluginHost;
use serde_json::{Value, json};

use crate::exec::Target;
use crate::feed::{Level, NetworkLevel};

/// The clear mark, relative to the plugin's out folder.
pub const STATE_FILE: &str = "state.json";
/// The clear mark, relative to the project.
pub const STATE_PATH: &str = ".rbrowser/out/console/state.json";
/// `console.bufferSize`'s bounds and default.
pub const BUFFER: (usize, usize, usize) = (100, 10_000, 1000);

/// The settings in effect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub levels: BTreeSet<Level>,
    pub buffer_size: usize,
    pub clear_on_navigate: bool,
    pub network: NetworkLevel,
    pub exec_target: Target,
    /// `console.mcpExec`: the MCP tool `exec` may run.
    pub mcp_exec: bool,
    /// `console.showRbrowserMessages`: what rBrowser's plugins logged shows too (RB-080-66).
    pub show_rbrowser: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            levels: Level::ALL
                .into_iter()
                .filter(|level| *level != Level::Debug)
                .collect(),
            buffer_size: BUFFER.2,
            clear_on_navigate: true,
            network: NetworkLevel::Errors,
            exec_target: Target::All,
            mcp_exec: false,
            show_rbrowser: false,
        }
    }
}

fn value(host: &mut dyn PluginHost, key: &str) -> Option<Value> {
    host.setting(key)
        .and_then(|text| serde_json::from_str(&text).ok())
}

impl Settings {
    /// The settings the host answers; the default for each key it has not or that does not read.
    #[must_use]
    pub fn read(host: &mut dyn PluginHost) -> Self {
        let mut settings = Self::default();
        if let Some(Value::Array(levels)) = value(host, "levels") {
            settings.levels = levels
                .iter()
                .filter_map(|level| level.as_str().and_then(Level::parse))
                .collect();
        }
        if let Some(size) = value(host, "bufferSize").and_then(|v| v.as_u64()) {
            let size = usize::try_from(size).unwrap_or(BUFFER.1);
            settings.buffer_size = size.clamp(BUFFER.0, BUFFER.1);
        }
        if let Some(Value::Bool(clear)) = value(host, "clearOnNavigate") {
            settings.clear_on_navigate = clear;
        }
        if let Some(Value::String(level)) = value(host, "networkLevel") {
            settings.network = NetworkLevel::parse(&level);
        }
        if let Some(Value::String(target)) = value(host, "execTarget") {
            settings.exec_target = if target == "active" {
                Target::Active
            } else {
                Target::All
            };
        }
        settings.mcp_exec = value(host, "mcpExec") == Some(Value::Bool(true));
        settings.show_rbrowser = value(host, "showRbrowserMessages") == Some(Value::Bool(true));
        settings
    }
}

/// The clear mark; `0` without one (none written, none readable).
#[must_use]
pub fn cleared_at(host: &mut dyn PluginHost) -> f64 {
    host.project_read(STATE_PATH)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|state| state["clearedAt"].as_f64())
        .filter(|at| at.is_finite())
        .unwrap_or(0.0)
}

/// Marks everything up to `newest` (milliseconds since the epoch) cleared, never later than the
/// clock allows; answers the mark. A mark that cannot be written is logged and answered all
/// the same: that answer's caller sees the console cleared, the next one does not.
pub fn clear(host: &mut dyn PluginHost, newest: f64) -> f64 {
    let ceiling = (host.now_unix_seconds() as f64 + 1.0) * 1000.0;
    let mark = if newest.is_finite() {
        newest.min(ceiling)
    } else {
        ceiling
    };
    let state = json!({ "clearedAt": mark }).to_string();
    if let Err(failure) = host.out_write(STATE_FILE, state.as_bytes()) {
        let code = failure.code.unwrap_or_default();
        host.log("warn", &format!("the clear mark was not written: {code}"));
    }
    mark
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
