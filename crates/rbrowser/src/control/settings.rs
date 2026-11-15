//! The settings side of the launch: the user settings file, the settings registry built from
//! the generated schemas (RB-010-02; the keys of rb-diag's `logs` and `updates.cefAgeWarning`
//! come from rb-config's schema, the one place they are defined), the leader's settings and the
//! log settings of rb-diag, and the config values the log must mask.

use std::path::PathBuf;
use std::time::Duration;

use rb_commands::ChordSettings;
use rb_config::{ConfigError, ConfigIssue, ProjectConfig, UserSettings};
use rb_ipc::SettingValue;
use rb_settings::{Origin, SchemaIssue, Scope, SettingsRegistry};
use serde_json::Value;

/// The user settings as the launch read them.
#[derive(Debug, Default)]
pub struct UserLoad {
    /// The settings in effect: the file's, a value that did not fit replaced by its default;
    /// all defaults when the file could not be read.
    pub settings: UserSettings,
    /// Where they live; `None` when the platform names no folder.
    pub path: Option<PathBuf>,
    /// Why the file could not be read (the defaults apply).
    pub error: Option<ConfigError>,
    /// The values that fell back to their defaults, each naming its key.
    pub warnings: Vec<ConfigIssue>,
}

impl UserLoad {
    /// Logs why the file could not be read and every value that fell back; once the log is up.
    pub fn log(&self) {
        if let Some(error) = &self.error {
            tracing::warn!(%error, "the user settings cannot be read; the defaults apply");
        }
        for warning in &self.warnings {
            tracing::warn!(key = %warning.path, "{}", warning.message);
        }
    }
}

/// Reads the user settings file of this machine.
pub fn load_user() -> UserLoad {
    let Some(path) = rb_config::user_settings_path() else {
        return UserLoad::default();
    };
    load_user_from(path)
}

/// Reads the user settings file at `path`.
pub fn load_user_from(path: PathBuf) -> UserLoad {
    match rb_config::load_user_settings_with_warnings(&path) {
        Ok((settings, warnings)) => UserLoad {
            settings,
            path: Some(path),
            error: None,
            warnings,
        },
        Err(error) => UserLoad {
            path: Some(path),
            error: Some(error),
            ..UserLoad::default()
        },
    }
}

/// The settings registry over the project and the user schema, and what it refused.
pub fn settings_registry() -> (SettingsRegistry, Vec<SchemaIssue>) {
    let mut registry = SettingsRegistry::new();
    let mut issues = Vec::new();
    for schema in [
        rb_config::project_schema(),
        rb_config::user_settings_schema(),
    ] {
        if let Err(found) = registry.register_schema(schema.as_value()) {
            issues.extend(found);
        }
    }
    (registry, issues)
}

/// Every user-scope key of `registry` with its value in `user` (the shell's `settings`
/// answer): origin `user` where the value differs from the schema default, else `default`.
/// The project keys come with the settings window (RB-010-08).
pub fn user_values(registry: &SettingsRegistry, user: &UserSettings) -> Vec<SettingValue> {
    let document = serde_json::to_value(user).unwrap_or_default();
    registry
        .iter()
        .filter(|def| def.scope_allowed.contains(&Scope::User))
        .map(|def| {
            let value = def
                .key
                .split('.')
                .try_fold(&document, |node, segment| node.get(segment))
                .cloned()
                .unwrap_or(Value::Null);
            let origin = if value == def.default_value() {
                Origin::Default
            } else {
                Origin::User
            };
            SettingValue {
                key: def.key.clone(),
                value,
                origin,
            }
        })
        .collect()
}

/// The leader's settings (`shortcuts.leaderTimeoutMs`, `shortcuts.disableInDevtools`); a
/// timeout of `0` is none (RB-080-18).
pub fn chord_settings(user: &UserSettings) -> ChordSettings {
    let timeout_ms = user.shortcuts.leader_timeout_ms;
    ChordSettings {
        leader_timeout: (timeout_ms != 0).then(|| Duration::from_millis(u64::from(timeout_ms))),
        disable_in_devtools: user.shortcuts.disable_in_devtools,
    }
}

/// The `logs` settings as rb-diag reads them; the defaults where a value does not fit.
pub fn log_settings(user: &UserSettings) -> rb_diag::LogSettings {
    serde_json::to_value(&user.logs)
        .and_then(serde_json::from_value)
        .unwrap_or_default()
}

/// Registers every request header value of `config` (breakpoints and emulation defaults) as a
/// secret, so the log masks it (RB-010-10).
pub fn register_config_secrets(config: &ProjectConfig) {
    let defaults = config.emulation.headers.values();
    let breakpoints = config
        .breakpoints
        .iter()
        .flat_map(|breakpoint| breakpoint.emulation.headers.values());
    for value in defaults.chain(breakpoints) {
        rb_diag::register_secret(value);
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
