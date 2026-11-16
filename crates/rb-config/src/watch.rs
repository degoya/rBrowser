//! The hook between a file watcher and the config. The watching itself (ReadDirectoryChangesW,
//! inotify in WSL, polling) is RB-030-05's; it hands every batch of changed paths to
//! [`reload_on_change`], which reloads only when a config file is among them.

use std::path::{Path, PathBuf};

use crate::jsonio::ConfigFile;
use crate::{CONFIG_DIR, ConfigError, LoadedConfig, load_project};

/// The files whose change reloads the project config at `root`.
pub fn watched_files(root: &Path) -> [PathBuf; 2] {
    [ConfigFile::Project.path(root), ConfigFile::Local.path(root)]
}

/// Whether `path` is one of the config files (`.rbrowser/config.json` or
/// `.rbrowser/config.local.json`), however the watcher spells the directories above it.
pub fn is_config_file(path: &Path) -> bool {
    [ConfigFile::Project, ConfigFile::Local]
        .iter()
        .any(|file| path.ends_with(Path::new(CONFIG_DIR).join(file.file_name())))
}

/// Reloads the config of `root` when one of `changed` is a config file and passes the result
/// - the new config or why it does not load - to `on_change`. Returns whether it reloaded.
pub fn reload_on_change(
    root: &Path,
    changed: &[PathBuf],
    on_change: impl FnOnce(Result<LoadedConfig, ConfigError>),
) -> bool {
    if !changed.iter().any(|path| is_config_file(path)) {
        return false;
    }
    on_change(load_project(root));
    true
}

#[cfg(test)]
#[path = "watch_tests.rs"]
mod tests;
