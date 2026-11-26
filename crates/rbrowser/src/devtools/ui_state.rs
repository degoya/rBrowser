//! DevTools' UI state (contract 0.1-w1b §4): the bounds of the DevTools window, kept in
//! `devtools.json` under [`rb_config::local_dir`]. Not a setting: it follows the user's hand,
//! and a broken or missing file only means the defaults. The dock's size was kept here too; it
//! is the settings `devtools.dockHeight` and `devtools.dockWidth` now (the splitter, RB-010-06),
//! and an older file's `dock` is ignored.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::runtime::Handle;

use crate::cdp::lock;

/// The file's name under the local folder.
pub const UI_STATE_FILE: &str = "devtools.json";
/// The smallest window kept, in pixels; anything smaller opens at the system's default.
pub const MIN_WINDOW: (u32, u32) = (320, 200);
/// The largest window side kept, in pixels.
pub const MAX_WINDOW_SIDE: u32 = 16_384;

/// Where the DevTools window was, in screen pixels (outer bounds).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowBounds {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
}

impl WindowBounds {
    /// Whether the bounds are worth opening at: neither tiny (a minimised window reports
    /// nonsense) nor absurdly large.
    pub fn usable(&self) -> bool {
        (MIN_WINDOW.0..=MAX_WINDOW_SIDE).contains(&self.width)
            && (MIN_WINDOW.1..=MAX_WINDOW_SIDE).contains(&self.height)
            && self.x.unsigned_abs() <= MAX_WINDOW_SIDE * 4
            && self.y.unsigned_abs() <= MAX_WINDOW_SIDE * 4
    }
}

/// What `devtools.json` holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UiState {
    /// The window's last bounds.
    pub window: Option<WindowBounds>,
}

impl UiState {
    /// The window bounds to open at, when they are usable.
    pub fn window_bounds(&self) -> Option<WindowBounds> {
        self.window.filter(WindowBounds::usable)
    }
}

/// The file in the local folder; `None` without one.
pub fn default_path() -> Option<PathBuf> {
    rb_config::local_dir().map(|dir| dir.join(UI_STATE_FILE))
}

/// Reads the state at `path`; a missing, unreadable or broken file gives the defaults.
pub fn load(path: &Path) -> UiState {
    if !path.exists() {
        return UiState::default();
    }
    match rb_config::read_json(path).and_then(|value| {
        serde_json::from_value::<UiState>(value).map_err(|error| rb_config::ConfigError::Parse {
            path: path.to_path_buf(),
            line: error.line(),
            column: error.column(),
            message: error.to_string(),
        })
    }) {
        Ok(state) => state,
        Err(error) => {
            tracing::warn!(%error, "the DevTools UI state was not read; using the defaults");
            UiState::default()
        }
    }
}

/// Writes `state` to `path`, creating its folder.
pub fn save(path: &Path, state: &UiState) -> Result<(), rb_config::ConfigError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|source| rb_config::ConfigError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
    }
    let value = serde_json::to_value(state).unwrap_or(Value::Null);
    rb_config::write_json(path, &value)
}

/// Writes the state to its file off the calling thread - the CEF UI thread, where the DevTools
/// window closes (RB-099-19) - one write at a time, the newest state winning: a state saved
/// while a write runs replaces any other still waiting.
pub struct Saver {
    path: PathBuf,
    slot: Mutex<Slot>,
}

#[derive(Default)]
struct Slot {
    /// The newest state not written yet.
    next: Option<UiState>,
    /// A writer runs (or is queued on the runtime).
    writing: bool,
}

impl Saver {
    /// A saver of the file at `path`.
    pub fn new(path: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            path,
            slot: Mutex::new(Slot::default()),
        })
    }

    /// Saves `state` on a blocking thread of `runtime`, after the writes before it. A runtime
    /// that shuts down refuses the task: the state is written on this thread then.
    pub fn save(self: &Arc<Self>, runtime: &Handle, state: UiState) {
        let start = {
            let mut slot = lock(&self.slot);
            slot.next = Some(state);
            !std::mem::replace(&mut slot.writing, true)
        };
        if start {
            let writer = Writer(self.clone());
            runtime.spawn_blocking(move || drop(writer));
        }
    }

    /// Whether a writer runs (tests).
    #[cfg(test)]
    pub fn writing(&self) -> bool {
        lock(&self.slot).writing
    }

    /// Writes the waiting states until none is left; the newest is the last.
    fn drain(&self) {
        loop {
            let state = {
                let mut slot = lock(&self.slot);
                match slot.next.take() {
                    Some(state) => state,
                    None => {
                        slot.writing = false;
                        return;
                    }
                }
            };
            if let Err(error) = save(&self.path, &state) {
                tracing::warn!(%error, "the DevTools UI state was not saved");
            }
        }
    }
}

/// The one writer of a [`Saver`]: it writes when it is dropped - on the blocking thread that
/// ran it, or on the caller's when the runtime refused and dropped the task unrun.
struct Writer(Arc<Saver>);

impl Drop for Writer {
    fn drop(&mut self) {
        self.0.drain();
    }
}

#[cfg(test)]
#[path = "ui_state_tests.rs"]
mod tests;
