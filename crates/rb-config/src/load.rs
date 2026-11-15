//! Loading a project's config: read, migrate in memory, apply the local-only rule and the trust
//! gate (closed unless `crate::trust` found the workspace trusted), merge, record origins,
//! validate. Loading never writes: a migrated file reaches the disk only when
//! it is saved ([`crate::write_config_file`], which backs the older file up first).

use std::path::Path;

use serde_json::Value;

use crate::jsonio::{ConfigFile, read_json};
use crate::local_only::{strip_local_only, strip_trust_gated};
use crate::merge::{merge, origins};
use crate::migrate::{migrate, version_of};
use crate::validate::validate;
use crate::{CURRENT_VERSION, ConfigError, ConfigIssue, LoadedConfig, MigrationNote};

/// The two project files as read and migrated in memory, before the local-only rule and the
/// trust gate: what the workspace trust hashes (`crate::trust`) and what [`finish`] loads.
#[derive(Clone, Debug)]
pub(crate) struct RawFiles {
    /// `config.json`.
    pub project: Value,
    /// `config.local.json`, when there is one.
    pub local: Option<Value>,
    /// What the migration changed.
    pub migrations: Vec<MigrationNote>,
}

/// See [`crate::load_project`]: the trust gate stays closed.
pub fn load_project(root: &Path) -> Result<LoadedConfig, ConfigError> {
    finish(root, read_files(root)?, false)
}

/// Reads and migrates both project files of `root`.
pub(crate) fn read_files(root: &Path) -> Result<RawFiles, ConfigError> {
    let mut migrations = Vec::new();
    let project_path = ConfigFile::Project.path(root);
    let project = read_object(&project_path)?;
    let project = migrate_value(project, true, &mut migrations)?;

    let local_path = ConfigFile::Local.path(root);
    let local = if local_path.is_file() {
        let local = read_object(&local_path)?;
        Some(migrate_value(local, false, &mut migrations)?)
    } else {
        None
    };
    Ok(RawFiles {
        project,
        local,
        migrations,
    })
}

/// Applies the local-only rule and - unless the workspace is trusted (`trusted`, decided by
/// `crate::trust` over the same `raw`) - the trust gate, merges, records origins, validates.
pub(crate) fn finish(
    root: &Path,
    raw: RawFiles,
    trusted: bool,
) -> Result<LoadedConfig, ConfigError> {
    let RawFiles {
        mut project,
        mut local,
        migrations,
    } = raw;
    let mut warnings = strip_local_only(&mut project);
    if !trusted {
        warnings.extend(strip_trust_gated(&mut project, crate::CONFIG_FILE));
        if let Some(local) = local.as_mut() {
            warnings.extend(strip_trust_gated(local, crate::LOCAL_CONFIG_FILE));
        }
    }
    let origins = origins(&project, local.as_ref());
    let mut merged = project;
    if let Some(local) = local {
        merge(&mut merged, local);
    }
    // A bad bookmark is dropped with a warning; the rest of the config stays in force.
    warnings.extend(crate::bookmarks::strip_invalid(&mut merged));
    // A value outside a schema's choice is kept, and named (RB-080-72).
    warnings.extend(crate::allowed::not_allowed(project_schema(), &merged));
    let config = validate(merged)?;
    Ok(LoadedConfig {
        root: root.to_path_buf(),
        config,
        origins,
        warnings,
        migrations,
    })
}

/// The project schema, built once.
fn project_schema() -> &'static Value {
    static SCHEMA: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    SCHEMA.get_or_init(|| Value::from(crate::project_schema()))
}

/// Reads `path` and refuses anything but a JSON object.
fn read_object(path: &Path) -> Result<Value, ConfigError> {
    let value = read_json(path)?;
    if value.is_object() {
        Ok(value)
    } else {
        Err(ConfigError::Invalid(vec![ConfigIssue {
            path: String::new(),
            key: "config.error.not_an_object".to_owned(),
            message: format!("{} must hold a JSON object", path.display()),
        }]))
    }
}

/// Migrates a file's value in memory when its `version` is older. `config.local.json` may leave
/// `version` out (`version_required` false): it then counts as current.
fn migrate_value(
    value: Value,
    version_required: bool,
    notes: &mut Vec<MigrationNote>,
) -> Result<Value, ConfigError> {
    let version = version_of(&value)?;
    let needs_migration = match version {
        Some(version) => version != CURRENT_VERSION,
        None => version_required,
    };
    if !needs_migration {
        return Ok(value);
    }
    let (migrated, file_notes) = migrate(value)?;
    notes.extend(file_notes);
    Ok(migrated)
}

#[cfg(test)]
#[path = "load_tests.rs"]
mod tests;
