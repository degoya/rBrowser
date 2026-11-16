//! Reading and writing the config files as plain JSON (no JSONC).
//!
//! The canonical form is `serde_json`'s pretty printer (two spaces) plus one trailing newline;
//! with `preserve_order` the keys keep their order, so a file in this form survives load and
//! save byte for byte. Writes go to a temporary file beside the target and are renamed over it.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::migrate::{version_of, write_backup};
use crate::{CONFIG_DIR, CURRENT_VERSION, ConfigError, LOCAL_CONFIG_FILE};

/// The `.rbrowser/.gitignore` written with the first config file: the local overrides, the
/// backups a save over an older version leaves (`*.bak-v*`) and the generated output stay out of
/// the repository.
pub const GITIGNORE: &str = "config.local.json\n*.bak-v*\nout/\n";

/// Which project config file to write.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConfigFile {
    /// `.rbrowser/config.json`, checked in.
    Project,
    /// `.rbrowser/config.local.json`, git-ignored.
    Local,
}

impl ConfigFile {
    /// The file name inside [`CONFIG_DIR`].
    pub const fn file_name(self) -> &'static str {
        match self {
            ConfigFile::Project => crate::CONFIG_FILE,
            ConfigFile::Local => LOCAL_CONFIG_FILE,
        }
    }

    /// The path of the file in the project at `root`.
    pub fn path(self, root: &Path) -> PathBuf {
        root.join(CONFIG_DIR).join(self.file_name())
    }
}

/// Reads `path` as one JSON value; a syntax error carries its 1-based line and column.
pub fn read_json(path: &Path) -> Result<Value, ConfigError> {
    let bytes = fs::read(path).map_err(|source| io_error(path, source))?;
    parse_json(path, &bytes)
}

/// Parses `bytes` (read from `path`) as one JSON value.
pub(crate) fn parse_json(path: &Path, bytes: &[u8]) -> Result<Value, ConfigError> {
    serde_json::from_slice(bytes).map_err(|error| ConfigError::Parse {
        path: path.to_path_buf(),
        line: error.line(),
        column: error.column(),
        message: error.to_string(),
    })
}

/// The canonical text of `value`: two-space indentation, key order kept, a trailing newline.
pub fn to_json_string(value: &Value) -> String {
    let mut text =
        serde_json::to_string_pretty(value).expect("a serde_json::Value always serializes");
    text.push('\n');
    text
}

/// Writes `value` to `path` in the canonical form, through a temporary file renamed over the
/// target so a reader never sees half a file.
pub fn write_json(path: &Path, value: &Value) -> Result<(), ConfigError> {
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temp = path.with_file_name(format!(".{file_name}.tmp-{}", std::process::id()));
    let written = fs::File::create(&temp).and_then(|mut file| {
        file.write_all(to_json_string(value).as_bytes())?;
        file.sync_all()
    });
    if let Err(source) = written {
        let _ = fs::remove_file(&temp);
        return Err(io_error(&temp, source));
    }
    fs::rename(&temp, path).map_err(|source| {
        let _ = fs::remove_file(&temp);
        io_error(path, source)
    })
}

/// Writes one config file of the project at `root`, creating `.rbrowser/` and, the first time,
/// `.rbrowser/.gitignore` ([`GITIGNORE`]). An existing `.gitignore` is left as it is. When the
/// file on disk has an older `version` (loading migrated it in memory only), it is copied to
/// `<file>.bak-v<n>` first.
pub fn write_config_file(root: &Path, file: ConfigFile, value: &Value) -> Result<(), ConfigError> {
    let dir = root.join(CONFIG_DIR);
    fs::create_dir_all(&dir).map_err(|source| io_error(&dir, source))?;
    let gitignore = dir.join(".gitignore");
    if !gitignore.exists() {
        fs::write(&gitignore, GITIGNORE).map_err(|source| io_error(&gitignore, source))?;
    }
    let path = file.path(root);
    if let Ok(old) = read_json(&path)
        && let Ok(Some(from)) = version_of(&old)
        && from < CURRENT_VERSION
    {
        write_backup(&path, from)?;
    }
    write_json(&path, value)
}

pub(crate) fn io_error(path: &Path, source: std::io::Error) -> ConfigError {
    ConfigError::Io {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
#[path = "jsonio_tests.rs"]
mod tests;
