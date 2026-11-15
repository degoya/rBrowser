//! The app's own folders: where the user settings, the trust store, the instance files, the logs
//! and the browser profiles live (contract 0.1-w1b, `docs/contracts-0.1-w1b.md`).
//!
//! Installed: the roaming folder is `%APPDATA%\rBrowser` (settings, `trust.json`), the local one
//! `%LOCALAPPDATA%\rBrowser` (logs, profiles, instances). Elsewhere (phase 2, the Linux tests)
//! `$XDG_CONFIG_HOME/rbrowser` and `$XDG_STATE_HOME/rBrowser` (each with its `$HOME` fallback).
//! **Portable** (RB-010-15): a file [`PORTABLE_MARKER`] beside the executable puts both under
//! `<exe dir>\data\` instead. Every path of the app is built from [`roaming_dir`] and
//! [`local_dir`], so the portable switch moves all of them at once.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::user_file::{UNIX_APP_DIR, WINDOWS_APP_DIR};

/// The file beside `rbrowser.exe` that switches the portable mode on.
pub const PORTABLE_MARKER: &str = "portable";
/// The folder beside the executable that holds everything in the portable mode.
pub const PORTABLE_DATA_DIR: &str = "data";
/// The local folder inside [`PORTABLE_DATA_DIR`].
pub const PORTABLE_LOCAL_DIR: &str = "local";
/// The local folder's name (`%LOCALAPPDATA%\rBrowser`, `$XDG_STATE_HOME/rBrowser`).
pub const LOCAL_APP_DIR: &str = "rBrowser";

/// The folder of the portable mode: the executable's folder when a file [`PORTABLE_MARKER`]
/// sits in it, else `None` (RB-010-15).
pub fn portable_root() -> Option<PathBuf> {
    portable_root_for(std::env::current_exe().ok().as_deref())
}

/// [`portable_root`] for the executable at `exe`: its folder when that holds a *file* named
/// [`PORTABLE_MARKER`]. A folder of that name, or an executable path without a folder, is no
/// portable mode.
pub fn portable_root_for(exe: Option<&Path>) -> Option<PathBuf> {
    let folder = exe?
        .parent()
        .filter(|folder| !folder.as_os_str().is_empty())?;
    folder
        .join(PORTABLE_MARKER)
        .is_file()
        .then(|| folder.to_path_buf())
}

/// The roaming folder (settings, trust store); `None` when the environment names none.
pub fn roaming_dir() -> Option<PathBuf> {
    roaming_dir_for(cfg!(windows), portable_root(), |name| {
        std::env::var_os(name)
    })
}

/// The local folder (logs, profiles, instance files); `None` when the environment names none.
pub fn local_dir() -> Option<PathBuf> {
    local_dir_for(cfg!(windows), portable_root(), |name| {
        std::env::var_os(name)
    })
}

/// [`roaming_dir`] with the platform, the portable root and the environment given.
pub fn roaming_dir_for(
    windows: bool,
    portable: Option<PathBuf>,
    var: impl Fn(&str) -> Option<OsString>,
) -> Option<PathBuf> {
    if let Some(root) = portable {
        return Some(root.join(PORTABLE_DATA_DIR));
    }
    let dir = |name: &str| set(&var, name);
    if windows {
        Some(dir("APPDATA")?.join(WINDOWS_APP_DIR))
    } else {
        Some(
            dir("XDG_CONFIG_HOME")
                .or_else(|| dir("HOME").map(|home| home.join(".config")))?
                .join(UNIX_APP_DIR),
        )
    }
}

/// [`local_dir`] with the platform, the portable root and the environment given.
pub fn local_dir_for(
    windows: bool,
    portable: Option<PathBuf>,
    var: impl Fn(&str) -> Option<OsString>,
) -> Option<PathBuf> {
    if let Some(root) = portable {
        return Some(root.join(PORTABLE_DATA_DIR).join(PORTABLE_LOCAL_DIR));
    }
    let dir = |name: &str| set(&var, name);
    let base = if windows {
        dir("LOCALAPPDATA")?
    } else {
        dir("XDG_STATE_HOME").or_else(|| dir("HOME").map(|home| home.join(".local/state")))?
    };
    Some(base.join(LOCAL_APP_DIR))
}

/// The variable `name` as a path; an empty one counts as absent.
fn set(var: &impl Fn(&str) -> Option<OsString>, name: &str) -> Option<PathBuf> {
    var(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
#[path = "dirs_tests.rs"]
mod tests;
