//! The user settings file: where it lives and how it loads.
//!
//! Windows (phase 1): `%APPDATA%\rBrowser\settings.json`. Elsewhere (phase 2):
//! `$XDG_CONFIG_HOME/rbrowser/settings.json`, else `$HOME/.config/rbrowser/settings.json`.
//! The platform is chosen with `cfg!`, so every branch compiles and is tested on Linux.

use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use rb_util::json_pointer::escape;
use serde_json::Value;

use crate::jsonio::{io_error, parse_json};
use crate::validate::pointer_of;
use crate::{CURRENT_VERSION, ConfigError, ConfigIssue, USER_SETTINGS_FILE, UserSettings};

/// The folder under `%APPDATA%` on Windows.
pub const WINDOWS_APP_DIR: &str = "rBrowser";
/// The folder under the XDG config directory elsewhere.
pub const UNIX_APP_DIR: &str = "rbrowser";

/// See [`crate::user_settings_path`]: inside [`crate::roaming_dir`], so the portable mode
/// (RB-010-15) moves it too.
pub fn user_settings_path() -> Option<PathBuf> {
    crate::roaming_dir().map(|dir| dir.join(USER_SETTINGS_FILE))
}

/// The settings path on Windows (`windows`) or elsewhere, reading the environment through
/// `var`. An empty variable counts as absent.
pub fn settings_path_for(windows: bool, var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let dir = |name: &str| {
        var(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    let base = if windows {
        dir("APPDATA")?.join(WINDOWS_APP_DIR)
    } else {
        dir("XDG_CONFIG_HOME")
            .or_else(|| dir("HOME").map(|home| home.join(".config")))?
            .join(UNIX_APP_DIR)
    };
    Some(base.join(USER_SETTINGS_FILE))
}

/// See [`crate::load_user_settings`].
pub fn load_user_settings(path: &Path) -> Result<UserSettings, ConfigError> {
    load_user_settings_with_warnings(path).map(|(settings, _)| settings)
}

/// How many invalid values one load drops before it gives up on the file.
const MAX_DROPPED: usize = 64;

/// See [`crate::load_user_settings_with_warnings`].
pub fn load_user_settings_with_warnings(
    path: &Path,
) -> Result<(UserSettings, Vec<ConfigIssue>), ConfigError> {
    let mut value = match std::fs::read(path) {
        Ok(bytes) => parse_json(path, &bytes)?,
        Err(error) if error.kind() == ErrorKind::NotFound => Value::Object(Default::default()),
        Err(source) => return Err(io_error(path, source)),
    };
    if let Some(found) = value.get("version").and_then(Value::as_u64)
        && found > u64::from(CURRENT_VERSION)
    {
        return Err(ConfigError::UnsupportedVersion {
            found: u32::try_from(found).unwrap_or(u32::MAX),
            supported: CURRENT_VERSION,
        });
    }
    let mut warnings = drop_outside_schema(&mut value);
    // What the schema check does not cover (a `oneOf`, a nested item) the types refuse: that
    // value is dropped too, until the rest deserializes.
    loop {
        let error = match serde_path_to_error::deserialize(value.clone()) {
            Ok(settings) => return Ok((settings, warnings)),
            Err(error) => error,
        };
        let pointer = pointer_of(error.path());
        let issue = invalid(pointer.clone(), error.inner().to_string());
        if pointer.is_empty() || warnings.len() >= MAX_DROPPED || !remove_at(&mut value, &pointer) {
            return Err(ConfigError::Invalid(vec![issue]));
        }
        warnings.push(issue);
    }
}

/// Removes every key whose value the user settings schema refuses (`type`, `enum`, `minimum`,
/// `maximum`: the check a settings write passes), so its default applies; one warning each.
fn drop_outside_schema(value: &mut Value) -> Vec<ConfigIssue> {
    let schema = crate::user_settings_schema();
    // The schema's own consistency is a test (`schema_tests`); without definitions only the
    // types check the file.
    let defs = rb_settings::defs_from_schema(schema.as_value()).unwrap_or_default();
    let mut warnings = Vec::new();
    for def in defs {
        let pointer: String = def
            .key
            .split('.')
            .map(|part| format!("/{}", escape(part)))
            .collect();
        let Some(found) = value.pointer(&pointer).filter(|found| !found.is_null()) else {
            continue;
        };
        if let Err(reason) = def.check_value(found)
            && remove_at(value, &pointer)
        {
            warnings.push(invalid(pointer, format!("`{}`: {reason}", def.key)));
        }
    }
    warnings
}

/// A warning for the value at `pointer`, which falls back to its default.
fn invalid(pointer: String, reason: String) -> ConfigIssue {
    ConfigIssue {
        path: pointer,
        key: "config.error.invalid_value".to_owned(),
        message: format!("{reason}; the default applies"),
    }
}

/// Removes the member or item at the JSON pointer `pointer`; `false` when there is none.
fn remove_at(value: &mut Value, pointer: &str) -> bool {
    let Some((parent, last)) = pointer.rsplit_once('/') else {
        return false;
    };
    let last = last.replace("~1", "/").replace("~0", "~");
    match value.pointer_mut(parent) {
        Some(Value::Object(object)) => object.remove(&last).is_some(),
        Some(Value::Array(items)) => match last.parse::<usize>() {
            Ok(index) if index < items.len() => {
                items.remove(index);
                true
            }
            _ => false,
        },
        _ => false,
    }
}

#[cfg(test)]
#[path = "user_file_tests.rs"]
mod tests;
