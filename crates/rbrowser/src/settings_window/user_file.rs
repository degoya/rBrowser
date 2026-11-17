//! The one way the app changes the user settings file (`settings.json`): the settings window,
//! DevTools' dock cycle (RB-010-06) and the recent projects (RB-010-11) all go through here, so
//! none of them writes over a file that exists but does not read, is no JSON object, or was
//! written by a newer rBrowser (a `version` this build does not know).

use std::path::Path;

use rb_config::{CURRENT_VERSION, ConfigError};
use serde_json::{Map, Value};

/// Why the user settings file was not changed.
#[derive(Debug, thiserror::Error)]
pub enum UserFileError {
    /// It exists but does not read as a JSON object of a version this build knows; it is kept
    /// as it is.
    #[error(
        "the user settings file does not read, is no object or comes from a newer rBrowser; it \
         is not written over"
    )]
    Kept,
    /// Reading the folder or writing the file failed.
    #[error(transparent)]
    Io(#[from] ConfigError),
}

/// Whether the file at `path` may be written: it does not exist, or it reads as a JSON object
/// of a version this build knows. The project files of the settings window are held to it too.
pub fn writable(path: &Path) -> bool {
    !path.exists() || current(path).is_ok()
}

/// Changes the file at `path` with `change` - read, changed, written back, every other key
/// kept; a missing file (and its folder) is made.
///
/// # Errors
///
/// [`UserFileError::Kept`] when the file may not be written ([`writable`]), else the write's.
pub fn update(
    path: &Path,
    change: impl FnOnce(&mut Map<String, Value>),
) -> Result<(), UserFileError> {
    let mut map = if path.exists() {
        current(path)?
    } else {
        Map::new()
    };
    change(&mut map);
    store(path, &Value::Object(map))
}

/// Writes `document` as the whole file at `path` (the settings window planned it over what it
/// read); a missing folder is made.
///
/// # Errors
///
/// [`UserFileError::Kept`] when the file may not be written ([`writable`]), else the write's.
pub fn write(path: &Path, document: &Value) -> Result<(), UserFileError> {
    if !writable(path) {
        return Err(UserFileError::Kept);
    }
    store(path, document)
}

/// The file's object, when it may be written over.
fn current(path: &Path) -> Result<Map<String, Value>, UserFileError> {
    let Ok(Value::Object(map)) = rb_config::read_json(path) else {
        return Err(UserFileError::Kept);
    };
    let known = map
        .get("version")
        .and_then(Value::as_u64)
        .is_none_or(|version| version <= u64::from(CURRENT_VERSION));
    if known {
        Ok(map)
    } else {
        Err(UserFileError::Kept)
    }
}

fn store(path: &Path, document: &Value) -> Result<(), UserFileError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|source| ConfigError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
    }
    rb_config::write_json(path, document)?;
    Ok(())
}

#[cfg(test)]
#[path = "user_file_tests.rs"]
mod tests;
