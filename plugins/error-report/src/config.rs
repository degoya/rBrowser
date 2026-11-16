//! The plugin's settings (`settings.schema.json`, Settings > Checks > Error Report) and its state.
//!
//! A component keeps nothing between calls, so its state is a file of its out folder,
//! `.rbrowser/out/error-report/state.json` (written with `out-write`, read back with
//! `project-read`): the clear mark - the time of the newest report when the report was cleared;
//! everything at or before it is left out - and whether a report waits to be published after a
//! reload ([`State::pending`]). A mark is never later than the clock, so a made-up capture cannot
//! hide the errors still to come.

use std::collections::BTreeMap;

use rb_plugin_api::{PluginHost, Severity};
use serde_json::{Value, json};

/// The state, relative to the plugin's out folder.
pub const STATE_FILE: &str = "state.json";
/// The state, relative to the project.
pub const STATE_PATH: &str = ".rbrowser/out/error-report/state.json";
/// The most ignore patterns read.
pub const MAX_IGNORE: usize = 100;
/// The longest ignore pattern read.
pub const MAX_PATTERN_CHARS: usize = 300;

/// A rule switched off or given another severity (`error-report.rules`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleSetting {
    Off,
    Severity(Severity),
}

/// The settings in effect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Per rule id: off or a severity; a rule not named keeps its own.
    pub rules: BTreeMap<String, RuleSetting>,
    /// Globs on an entry's URL, source and text; a match leaves the entry out.
    pub ignore: Vec<String>,
    /// The command `run` reloads the views first.
    pub reload_before_report: bool,
    /// Entries from other sites than the view's page count.
    pub third_party: bool,
    /// A view's entries from before its page last loaded are left out.
    pub since_navigation: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            rules: BTreeMap::new(),
            ignore: Vec::new(),
            reload_before_report: false,
            third_party: true,
            since_navigation: true,
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
        if let Some(Value::Object(rules)) = value(host, "rules") {
            settings.rules = rules
                .iter()
                .filter_map(|(rule, setting)| {
                    let setting = match setting.as_str()? {
                        "off" => RuleSetting::Off,
                        name => RuleSetting::Severity(Severity::parse(name)?),
                    };
                    Some((rule.clone(), setting))
                })
                .collect();
        }
        if let Some(Value::Array(patterns)) = value(host, "ignore") {
            settings.ignore = patterns
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|p| !p.is_empty() && p.chars().count() <= MAX_PATTERN_CHARS)
                .take(MAX_IGNORE)
                .map(str::to_owned)
                .collect();
        }
        if let Some(Value::Bool(reload)) = value(host, "reloadBeforeReport") {
            settings.reload_before_report = reload;
        }
        if let Some(Value::Bool(third)) = value(host, "thirdParty") {
            settings.third_party = third;
        }
        if let Some(Value::Bool(since)) = value(host, "sinceNavigation") {
            settings.since_navigation = since;
        }
        settings
    }
}

/// The state on disk.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct State {
    /// Entries at or before this time (milliseconds since the epoch) are cleared; `0` for none.
    pub cleared_at: f64,
    /// A reload ran: the next call publishes the report (files and findings).
    pub pending: bool,
}

impl State {
    /// The state written last; the default when none reads.
    #[must_use]
    pub fn read(host: &mut dyn PluginHost) -> Self {
        let Some(state) = host
            .project_read(STATE_PATH)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        else {
            return Self::default();
        };
        Self {
            cleared_at: state["clearedAt"]
                .as_f64()
                .filter(|at| at.is_finite())
                .unwrap_or(0.0),
            pending: state["pending"] == json!(true),
        }
    }

    /// Writes the state; a state that cannot be written is logged: the next call does not see it.
    pub fn write(self, host: &mut dyn PluginHost) {
        let text = json!({ "clearedAt": self.cleared_at, "pending": self.pending }).to_string();
        if let Err(failure) = host.out_write(STATE_FILE, text.as_bytes()) {
            let code = failure.code.unwrap_or_default();
            host.log(
                "warn",
                &format!("the error report state was not written: {code}"),
            );
        }
    }
}

/// The clear mark for a capture whose newest entry is `newest`: never later than the clock.
#[must_use]
pub fn clear_mark(host: &mut dyn PluginHost, newest: f64) -> f64 {
    #[allow(
        clippy::cast_precision_loss,
        reason = "seconds since the epoch stay far below 2^52"
    )]
    let ceiling = (host.now_unix_seconds() as f64 + 1.0) * 1000.0;
    if newest.is_finite() && newest > 0.0 {
        newest.min(ceiling)
    } else {
        ceiling
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
