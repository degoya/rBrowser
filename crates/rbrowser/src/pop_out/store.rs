//! Where each breakpoint's pop-out window was (RB-080-14, `popOut.rememberBounds`): its client
//! area in screen DIP, kept in `pop-out.json` under [`rb_config::local_dir`] like the DevTools
//! window's bounds (`crate::devtools::ui_state`). Not a setting: it follows the person's hand,
//! and a broken or missing file only means the windows open at their real size again.
//!
//! The commands read the file through a cache (read once per process, warmed off the UI thread
//! at start), and every change goes load-remember-save under one lock, so two windows closed
//! at once both keep their bounds (Charge 10 review L6).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::bounds::ScreenRect;
use crate::cdp::lock;
pub use crate::devtools::WindowBounds;

/// The file's name under the local folder.
pub const STATE_FILE: &str = "pop-out.json";
/// The most breakpoints kept; a new one beyond drops the first by name.
pub const MAX_ENTRIES: usize = 64;

/// What `pop-out.json` holds.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PopOutState {
    /// The client area of each breakpoint's window, by breakpoint id.
    pub windows: BTreeMap<String, WindowBounds>,
}

impl PopOutState {
    /// The client area to open `breakpoint`'s window at, when it is usable.
    pub fn bounds(&self, breakpoint: &str) -> Option<ScreenRect> {
        let bounds = self
            .windows
            .get(breakpoint)
            .filter(|bounds| bounds.usable())?;
        Some(ScreenRect {
            x: bounds.x,
            y: bounds.y,
            width: i32::try_from(bounds.width).ok()?,
            height: i32::try_from(bounds.height).ok()?,
        })
    }

    /// Keeps `client` for `breakpoint`, unless it is unusable (a minimised window).
    pub fn remember(&mut self, breakpoint: &str, client: ScreenRect) {
        let (Ok(width), Ok(height)) = (u32::try_from(client.width), u32::try_from(client.height))
        else {
            return;
        };
        let bounds = WindowBounds {
            x: client.x,
            y: client.y,
            width,
            height,
        };
        if !bounds.usable() {
            return;
        }
        if !self.windows.contains_key(breakpoint) {
            while self.windows.len() >= MAX_ENTRIES {
                self.windows.pop_first();
            }
        }
        self.windows.insert(breakpoint.to_owned(), bounds);
    }
}

/// The file in the local folder; `None` without one.
pub fn default_path() -> Option<PathBuf> {
    rb_config::local_dir().map(|dir| dir.join(STATE_FILE))
}

/// Reads the state at `path`; a missing, unreadable or broken file gives the defaults.
pub fn load(path: &Path) -> PopOutState {
    if !path.exists() {
        return PopOutState::default();
    }
    let read = rb_config::read_json(path).and_then(|value| {
        serde_json::from_value::<PopOutState>(value).map_err(|error| {
            rb_config::ConfigError::Parse {
                path: path.to_path_buf(),
                line: error.line(),
                column: error.column(),
                message: error.to_string(),
            }
        })
    });
    match read {
        Ok(state) => state,
        Err(error) => {
            tracing::warn!(%error, "the pop-out windows' bounds were not read; using none");
            PopOutState::default()
        }
    }
}

/// Writes `state` to `path`, creating its folder.
pub fn save(path: &Path, state: &PopOutState) -> Result<(), rb_config::ConfigError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|source| rb_config::ConfigError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
    }
    let value = serde_json::to_value(state).unwrap_or(Value::Null);
    rb_config::write_json(path, &value)
}

/// The state of each file, as read once and changed since.
static CACHE: Mutex<BTreeMap<PathBuf, PopOutState>> = Mutex::new(BTreeMap::new());
/// Held from a change to its write: the writes leave in the order the changes were made.
static WRITE: Mutex<()> = Mutex::new(());

/// Runs `change` on the cached state of `path`, reading the file first when it is not cached;
/// the read happens outside the cache's lock.
fn with_cached<R>(path: &Path, change: impl FnOnce(&mut PopOutState) -> R) -> R {
    let cached = lock(&CACHE).contains_key(path);
    let loaded = (!cached).then(|| load(path));
    let mut cache = lock(&CACHE);
    let state = cache
        .entry(path.to_path_buf())
        .or_insert_with(|| loaded.unwrap_or_default());
    change(state)
}

/// Reads the file at `path` into the cache; for a blocking thread at start.
pub fn warm(path: &Path) {
    with_cached(path, |_| ());
}

/// The client area to open `breakpoint`'s window at, from the cached state of `path`.
pub fn remembered(path: &Path, breakpoint: &str) -> Option<ScreenRect> {
    with_cached(path, |state| state.bounds(breakpoint))
}

/// Keeps `client` for `breakpoint` in the state of `path` and writes the file; off the UI
/// thread. Serialised, so no window's bounds are lost to another's write.
///
/// # Errors
///
/// The file cannot be written.
pub fn remember_and_save(
    path: &Path,
    breakpoint: &str,
    client: ScreenRect,
) -> Result<(), rb_config::ConfigError> {
    let _write = lock(&WRITE);
    let snapshot = with_cached(path, |state| {
        state.remember(breakpoint, client);
        state.clone()
    });
    save(path, &snapshot)
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
