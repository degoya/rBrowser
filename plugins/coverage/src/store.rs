//! What the plugin keeps in `.rbrowser/out/coverage/` - a component keeps no state between calls:
//! `session.json` (an interaction session's views and what its takes added up to, or a load run
//! going on), `last.json` (the last finished measurement, which the check run and the panel
//! read) and `sources/<hash>.css` (the style sheets' text, for the panel's source view).

use rb_plugin_api::{Failure, FailureKind, PluginHost, codes};
use serde::{Deserialize, Serialize};

use crate::model::{Mode, Run};
use crate::resolve::Cache;

/// The plugin's out folder, as `project.read` names it.
pub const OUT: &str = ".rbrowser/out/coverage/";
pub const SESSION: &str = "session.json";
pub const LAST: &str = "last.json";
/// The largest file `out.write` takes.
pub const MAX_WRITE: usize = 8 * 1024 * 1024;
/// A load run not finished after this many seconds is taken as gone (the call was cut off).
pub const LOAD_STALE_SECONDS: u64 = 15 * 60;

/// What runs now.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    /// `load` or `interaction` while one runs; none when idle.
    #[serde(default)]
    pub running: Option<Mode>,
    #[serde(default)]
    pub started: u64,
    /// An interaction session's takes, added up.
    #[serde(default)]
    pub run: Run,
}

impl Session {
    /// The mode running at `now`; a load run past [`LOAD_STALE_SECONDS`] has ended.
    #[must_use]
    pub fn active(&self, now: u64) -> Option<Mode> {
        match self.running {
            Some(Mode::Load) if now.saturating_sub(self.started) > LOAD_STALE_SECONDS => None,
            running => running,
        }
    }
}

fn read<T: for<'de> Deserialize<'de>>(host: &mut dyn PluginHost, name: &str) -> Option<T> {
    let bytes = host.project_read(&format!("{OUT}{name}")).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write<T: Serialize>(host: &mut dyn PluginHost, name: &str, value: &T) -> Result<(), Failure> {
    let bytes = serde_json::to_vec(value)
        .map_err(|_| Failure::coded(FailureKind::Other, codes::BAD_OUTPUT))?;
    host.out_write(name, &bytes)
}

/// The session, idle when none was stored.
pub fn session(host: &mut dyn PluginHost) -> Session {
    read(host, SESSION).unwrap_or_default()
}

/// Stores the session.
///
/// # Errors
///
/// The host's failure to write.
pub fn save_session(host: &mut dyn PluginHost, session: &Session) -> Result<(), Failure> {
    write(host, SESSION, session)
}

/// Marks the session idle; a failure is logged, never raised (the run's answer matters more).
pub fn clear_session(host: &mut dyn PluginHost) {
    if save_session(host, &Session::default()).is_err() {
        host.log("warn", "coverage: the session could not be cleared");
    }
}

/// The last finished measurement.
pub fn last(host: &mut dyn PluginHost) -> Option<Run> {
    read(host, LAST)
}

/// Stores the last measurement; when it is too large, without its rule and function lists.
///
/// # Errors
///
/// The host's failure to write.
pub fn save_last(host: &mut dyn PluginHost, run: &Run) -> Result<(), Failure> {
    let full = serde_json::to_vec(run).unwrap_or_default();
    if full.len() <= MAX_WRITE {
        return host.out_write(LAST, &full);
    }
    let mut lean = run.clone();
    for view in &mut lean.views {
        for file in &mut view.files {
            file.rules.clear();
            file.functions.clear();
        }
    }
    host.log(
        "info",
        "coverage: the rule and function lists were left out of last.json (too large)",
    );
    write(host, LAST, &lean)
}

/// FNV-1a, 64 bit, as hex: the name of a stored source.
#[must_use]
pub fn hash(text: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// The path of a stored source below the out folder.
#[must_use]
pub fn source_path(key: &str) -> String {
    format!("sources/{key}.css")
}

/// Stores a sheet's text once per call; its key, none when too large or not written.
pub fn write_source(host: &mut dyn PluginHost, text: &str, cache: &mut Cache) -> Option<String> {
    if text.len() > MAX_WRITE {
        return None;
    }
    let key = hash(text);
    if cache.written.contains(&key) {
        return Some(key);
    }
    host.out_write(&source_path(&key), text.as_bytes()).ok()?;
    cache.written.insert(key.clone());
    Some(key)
}

/// A stored source's text.
pub fn read_source(host: &mut dyn PluginHost, key: &str) -> Option<String> {
    // Only names this plugin made: 16 hex digits.
    if key.len() != 16 || !key.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let bytes = host
        .project_read(&format!("{OUT}{}", source_path(key)))
        .ok()?;
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
