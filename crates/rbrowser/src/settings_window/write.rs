//! Planning a settings write (`setSetting`): every check before anything is written, and the
//! file's new content. Portable and free of I/O; `apply.rs` reads the files and writes the plan.
//!
//! The checks, in order: the key exists; an MCP caller may not touch an `x-protected` key
//! (`check_mcp_access`) nor what `config_set` refuses (`rb_mcp::config_edit::write_refusal`);
//! a self-update key is locked in a build without self-update (`crate::updates`);
//! `shortcuts.bindings` belongs to the shortcut editor; the scope is one
//! the key allows (a local-only key never goes to the project); a project scope needs a project;
//! the value passes `SettingDef::check_value` and then the deep check of its whole schema; the
//! result deserializes into the typed settings or config, and a project config passes
//! `rb_config::check`. `null` resets: the key leaves the file.

use rb_config::{ConfigFile, ProjectConfig, UserSettings};
use rb_ipc::SettingIssue;
use rb_settings::{Scope, SettingDef, SettingsRegistry};
use serde_json::Value;

use super::deepcheck;
use super::docedit;
use super::values::Documents;

/// The key the shortcut editor writes; the generic write refuses it.
pub const BINDINGS_KEY: &str = "shortcuts.bindings";

/// The i18n keys of the refusals (besides the deep check's, `deepcheck::reason`).
pub mod reason {
    /// No setting has the key.
    pub const UNKNOWN_KEY: &str = "settings.rejected.unknown_key";
    /// An MCP caller asked for an `x-protected` key.
    pub const PROTECTED: &str = "settings.rejected.protected";
    /// `shortcuts.bindings` is the shortcut editor's.
    pub const SHORTCUT_EDITOR: &str = "settings.rejected.shortcut_editor";
    /// A local-only key cannot be stored in the project.
    pub const LOCAL_ONLY: &str = "settings.rejected.local_only";
    /// The key does not allow the scope.
    pub const SCOPE: &str = "settings.rejected.scope";
    /// A project scope without an open project.
    pub const NO_PROJECT: &str = "settings.rejected.no_project";
    /// The value does not fit the key.
    pub const INVALID: &str = "settings.rejected.invalid";
    /// The file could not be written.
    pub const NOT_SAVED: &str = "settings.rejected.not_saved";
    /// A self-update key in a build without self-update (RB-090-02).
    pub const UPDATES_INACTIVE: &str = crate::updates::INACTIVE_KEY;
}

/// Who writes: the settings window (a person) or an MCP client (RB-030-03).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Caller {
    /// The settings window.
    Shell,
    /// An MCP client: `x-protected` keys and what `config_set` refuses are refused.
    Mcp,
}

/// What to write.
#[derive(Clone, Debug, PartialEq)]
pub enum Plan {
    /// The user settings file's new content and the settings in effect after it.
    User {
        /// The file's new JSON.
        document: Value,
        /// The settings in effect.
        settings: Box<UserSettings>,
    },
    /// A project file's new content.
    Project {
        /// `config.json` or `config.local.json`.
        file: ConfigFile,
        /// The file's new JSON.
        document: Value,
    },
}

/// Checks the write of `value` (`null`: reset) to `key` in `scope` and returns what to write,
/// or every reason it is refused.
pub fn plan(
    registry: &SettingsRegistry,
    documents: &Documents,
    key: &str,
    value: &Value,
    scope: Scope,
    caller: Caller,
) -> Result<Plan, Vec<SettingIssue>> {
    let def = registry
        .get(key)
        .ok_or_else(|| refused(reason::UNKNOWN_KEY))?;
    if caller == Caller::Mcp && mcp_refuses(registry, key, value) {
        return Err(refused(reason::PROTECTED));
    }
    // Self-update is not in this build (RB-090-02): its keys are shown locked and not written.
    if crate::updates::is_locked(key) {
        return Err(refused(reason::UPDATES_INACTIVE));
    }
    if key == BINDINGS_KEY {
        return Err(refused(reason::SHORTCUT_EDITOR));
    }
    if def.local_only && scope != Scope::Local {
        return Err(refused(reason::LOCAL_ONLY));
    }
    if !def.scope_allowed.contains(&scope) {
        return Err(refused(reason::SCOPE));
    }
    if !value.is_null() {
        def.check_value(value)
            .map_err(|_| refused(reason::INVALID))?;
        let issues = deepcheck::check(&def.schema, value);
        if !issues.is_empty() {
            return Err(issues);
        }
    }
    match scope {
        Scope::User => plan_user(def, documents, value),
        Scope::Project | Scope::Local => plan_project(def, documents, value, scope),
    }
}

/// Whether an MCP client may not write `value` to `key`: an `x-protected` key, or what
/// `config_set` refuses (`rb_mcp::config_edit::write_refusal`: the keys workspace trust guards,
/// request headers anywhere in the value, the screenshot folder) - one list for both tools.
fn mcp_refuses(registry: &SettingsRegistry, key: &str, value: &Value) -> bool {
    registry.check_mcp_access(key).is_err()
        || rb_mcp::config_edit::write_refusal(key, value).is_some()
}

fn plan_user(
    def: &SettingDef,
    documents: &Documents,
    value: &Value,
) -> Result<Plan, Vec<SettingIssue>> {
    let old = documents.effective(def);
    let mut document = documents
        .user_file
        .clone()
        .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
    let mut effective = documents.user.clone();
    let new = if value.is_null() {
        docedit::remove(&mut document, &def.key);
        def.default_value()
    } else {
        docedit::set(&mut document, &def.key, &old, value).map_err(|_| refused(reason::INVALID))?;
        value.clone()
    };
    docedit::set(&mut effective, &def.key, &old, &new).map_err(|_| refused(reason::INVALID))?;
    let settings: UserSettings =
        serde_json::from_value(effective).map_err(|_| refused(reason::INVALID))?;
    Ok(Plan::User {
        document,
        settings: Box::new(settings),
    })
}

fn plan_project(
    def: &SettingDef,
    documents: &Documents,
    value: &Value,
    scope: Scope,
) -> Result<Plan, Vec<SettingIssue>> {
    let project = documents
        .project
        .as_ref()
        .ok_or_else(|| refused(reason::NO_PROJECT))?;
    let file = if scope == Scope::Local {
        ConfigFile::Local
    } else {
        ConfigFile::Project
    };
    let old = documents.effective(def);
    let mut document = documents
        .file(scope)
        .cloned()
        .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
    if value.is_null() {
        docedit::remove(&mut document, &def.key);
    } else {
        docedit::set(&mut document, &def.key, &old, value).map_err(|_| refused(reason::INVALID))?;
    }
    let (project_file, local_file) = match file {
        ConfigFile::Project => (&document, project.local_file.as_ref()),
        ConfigFile::Local => (&project.project_file, Some(&document)),
    };
    validate_project(project_file, local_file, &def.key)?;
    Ok(Plan::Project { file, document })
}

/// Loads the two files as `rb_config::load_project` would - migrated, the local-only keys out
/// of `config.json`, the trust-gated keys out of both, merged - and checks the result, a
/// breakpoint's size against the device presets in effect included. Issue paths below `key` are
/// relative to it; MCP's `config_set` passes `""` (RB-030-01).
pub fn validate_project(
    project_file: &Value,
    local_file: Option<&Value>,
    key: &str,
) -> Result<(), Vec<SettingIssue>> {
    let (mut merged, _) =
        rb_config::migrate(project_file.clone()).map_err(|_| refused(reason::INVALID))?;
    for gated in rb_config::LOCAL_ONLY_KEYS
        .iter()
        .chain(rb_config::TRUST_GATED_KEYS)
    {
        docedit::remove(&mut merged, gated);
    }
    if let Some(local) = local_file {
        let mut local = local.clone();
        for gated in rb_config::TRUST_GATED_KEYS {
            docedit::remove(&mut local, gated);
        }
        rb_config::merge(&mut merged, local);
    }
    let config: ProjectConfig =
        serde_json::from_value(merged).map_err(|_| refused(reason::INVALID))?;
    let prefix = format!("/{}", key.replace('.', "/"));
    // A size taken from a device preset needs the presets in effect (RB-010-12).
    let devices = crate::emulation::device_list();
    let issues: Vec<SettingIssue> = rb_config::check(&config)
        .into_iter()
        .chain(rb_config::check_viewports(&config, &devices))
        .map(|issue| SettingIssue {
            path: relative(&issue.path, &prefix),
            message_key: issue.key,
        })
        .collect();
    if issues.is_empty() {
        Ok(())
    } else {
        Err(issues)
    }
}

/// `path` below the key at `prefix` when it lies there; else the whole pointer (a rule between
/// keys, such as a cell naming a breakpoint).
fn relative(path: &str, prefix: &str) -> String {
    match path.strip_prefix(prefix) {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => rest.to_owned(),
        _ => path.to_owned(),
    }
}

/// One refusal of the key as a whole.
pub fn refused(message_key: &str) -> Vec<SettingIssue> {
    vec![SettingIssue {
        path: String::new(),
        message_key: message_key.to_owned(),
    }]
}

#[cfg(test)]
#[path = "write_tests.rs"]
mod tests;
