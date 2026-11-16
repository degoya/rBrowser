//! Moving a config of an older `version` to [`CURRENT_VERSION`].
//!
//! [`STEPS`] holds one function per version step; `migrate` runs them in order from the file's
//! version. A step works on partial values too (`config.local.json` may hold a few keys only).
//! Loading migrates in memory only; saving over a file of an older version writes the original
//! to a backup first ([`write_backup`], called by [`crate::write_config_file`]).

use std::fs;
use std::io::{ErrorKind, Write as _};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::jsonio::io_error;
use crate::{CURRENT_VERSION, ConfigError, ConfigIssue, MigrationNote};

/// A step from version `n` to `n + 1`: the changed value and what changed.
type Step = fn(Value) -> (Value, Vec<MigrationNote>);

/// The migration steps; index `n` migrates version `n` to `n + 1`.
const STEPS: &[Step] = &[v0_to_v1];

/// The `version` of a config value: `Ok(None)` when absent, an issue when it is not a
/// non-negative integer.
pub fn version_of(value: &Value) -> Result<Option<u32>, ConfigError> {
    match value.get("version") {
        None => Ok(None),
        Some(version) => version
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .map(Some)
            .ok_or_else(|| {
                ConfigError::Invalid(vec![ConfigIssue {
                    path: "/version".to_owned(),
                    key: "config.error.invalid_version".to_owned(),
                    message: format!("`version` must be a whole number, found {version}"),
                }])
            }),
    }
}

/// Migrates `value` from its `version` to [`CURRENT_VERSION`]; a current value comes back
/// unchanged with no notes.
pub fn migrate(value: Value) -> Result<(Value, Vec<MigrationNote>), ConfigError> {
    let Some(mut version) = version_of(&value)? else {
        return Err(ConfigError::Invalid(vec![ConfigIssue {
            path: "/version".to_owned(),
            key: "config.error.missing_version".to_owned(),
            message: "the config has no `version`".to_owned(),
        }]));
    };
    if version > CURRENT_VERSION {
        return Err(ConfigError::UnsupportedVersion {
            found: version,
            supported: CURRENT_VERSION,
        });
    }
    let mut value = value;
    let mut notes = Vec::new();
    while version < CURRENT_VERSION {
        let step = STEPS
            .get(version as usize)
            .expect("STEPS holds one step per version below CURRENT_VERSION");
        let (next, step_notes) = step(value);
        value = next;
        notes.extend(step_notes);
        version += 1;
        if let Some(object) = value.as_object_mut() {
            object.insert("version".to_owned(), Value::from(version));
        }
    }
    Ok((value, notes))
}

/// Version 0 marks the drafts written before the first release; their keys are those of
/// version 1, so only the number moves.
fn v0_to_v1(value: Value) -> (Value, Vec<MigrationNote>) {
    let note = MigrationNote {
        from: 0,
        to: 1,
        key: "config.migration.v0_to_v1".to_owned(),
        message: "draft config (version 0) taken over as version 1; no key changed".to_owned(),
    };
    (value, vec![note])
}

/// Copies `path` to `<file>.bak-v<from>` beside it before a save rewrites it. An existing
/// backup is never overwritten: the next free `<file>.bak-v<from>-<n>` is used instead.
pub fn write_backup(path: &Path, from: u32) -> Result<PathBuf, ConfigError> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let bytes = fs::read(path).map_err(|source| io_error(path, source))?;
    let mut attempt = 1u32;
    loop {
        let suffix = if attempt == 1 {
            String::new()
        } else {
            format!("-{attempt}")
        };
        let backup = path.with_file_name(format!("{name}.bak-v{from}{suffix}"));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&backup)
        {
            Ok(mut file) => {
                file.write_all(&bytes)
                    .and_then(|()| file.sync_all())
                    .map_err(|source| io_error(&backup, source))?;
                return Ok(backup);
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => attempt += 1,
            Err(source) => return Err(io_error(&backup, source)),
        }
    }
}

#[cfg(test)]
#[path = "migrate_tests.rs"]
mod tests;
