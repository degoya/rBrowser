//! The core's side of the settings window: reading the three files, writing a planned change,
//! and what follows a change - the user settings in effect, the log level, `settingsChanged` to
//! every shell page. A file that exists but cannot be read is never written over. The shell's
//! `setSetting` arrives on the CEF UI thread and is answered off it ([`set_setting_off_thread`]).

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use rb_config::{ConfigFile, UserSettings};
use rb_ipc::{CoreToShell, Notification, NotificationLevel, PathKind, SettingIssue};
use rb_settings::Scope;
use serde_json::{Value, json};

use super::user_file::{self, writable};
use super::values::{Documents, ProjectDocuments, schema_payload, values};
use super::write::{self, Caller, Plan, reason};
use super::{dynamic, lane};
use crate::cdp::lock;
use crate::control::Core;
use crate::control::settings::log_settings;
use crate::host::ui::PathPick;
use crate::ipc::Failure;

/// The i18n key of the notice that a user setting holds for the session only.
pub const NOT_SAVED_KEY: &str = "settings.notice.not_saved";
/// The i18n key of the path dialog's title when the page names none.
pub const PICK_TITLE_KEY: &str = "settings.pick.title";

/// The running logger's handle, handed over by the CEF start (`set_log_handle`).
static LOG: OnceLock<rb_diag::LogHandle> = OnceLock::new();

/// One change at a time, process-wide: [`set_setting`] reads the files, plans over what it read
/// and writes them back, and its callers come on any thread (the shell's lane, MCP, a command
/// handler). Two at once would each write over the other's key.
static ONE_CHANGE: Mutex<()> = Mutex::new(());

/// Keeps the logger's handle, so a change of `logs.level` applies at once. Called once at start.
pub fn set_log_handle(handle: rb_diag::LogHandle) {
    if LOG.set(handle).is_err() {
        tracing::debug!("the log handle was handed over twice");
    }
}

/// The documents the values resolve over, read now.
pub fn documents(core: &Core) -> Documents {
    let user = serde_json::to_value(core.user_settings()).unwrap_or_else(|_| json!({}));
    let user_file = core
        .user_settings_path()
        .filter(|path| path.is_file())
        .and_then(|path| rb_config::read_json(path).ok());
    let project = core
        .host()
        .project_root()
        .map(|root| project_documents(core, root));
    Documents {
        user,
        user_file,
        project,
    }
}

fn project_documents(core: &Core, root: PathBuf) -> ProjectDocuments {
    let project_file =
        rb_config::read_json(&ConfigFile::Project.path(&root)).unwrap_or_else(|_| json!({}));
    let local_path = ConfigFile::Local.path(&root);
    let local_file = if local_path.is_file() {
        rb_config::read_json(&local_path).ok()
    } else {
        None
    };
    // Read from disk, so a write shows before the watcher has reloaded the grid; behind the
    // trust gate (RB-010-09) as the shell holds it, so a gated key shows only where it applies
    // and no request hashes the protected folders again.
    let trusted = core.host().shell_state().trusted;
    let config = rb_config::trust::load_project_with_trust(&root, trusted)
        .map_or_else(|_| core.host().config(), |loaded| loaded.config);
    ProjectDocuments {
        effective: serde_json::to_value(config).unwrap_or_else(|_| json!({})),
        root,
        project_file,
        local_file,
    }
}

/// The `settings` answer: every key's value and origin, and the schema when asked - with the
/// dynamic fields' data from the core (RB-080-32). The values of the `x-options` sources come
/// with `getSettingSources` (RB-080-72).
pub fn settings_answer(core: &Core, include_schema: bool) -> CoreToShell {
    let documents = documents(core);
    let registry = core.settings_registry();
    CoreToShell::Settings {
        values: values(registry, &documents),
        schema: include_schema.then(|| {
            let mut payload = schema_payload(registry, &documents);
            for (key, data) in dynamic::fields(core) {
                dynamic::add(&mut payload, key, data);
            }
            payload
        }),
    }
}

/// `setSetting` (and later MCP's `settings_set`): checks, writes and applies `value` (`null`
/// resets) at `key` in `scope`. Answers `ack`, or `settingRejected` with the reasons.
pub fn set_setting(
    core: &Core,
    key: &str,
    value: &Value,
    scope: Scope,
    caller: Caller,
) -> CoreToShell {
    let _one_at_a_time = lock(&ONE_CHANGE);
    let documents = documents(core);
    // A program setting names an existing file (RB-080-58) - looked at only after the plan's
    // checks, so a caller refused the key (MCP: `x-protected`) learns nothing about the disk.
    let plan = write::plan(
        core.settings_registry(),
        &documents,
        key,
        value,
        scope,
        caller,
    )
    .and_then(
        |plan| match super::program::refusal(key, value, std::path::Path::is_file) {
            Some(reason) => Err(write::refused(reason)),
            None => Ok(plan),
        },
    );
    match plan {
        Err(issues) => rejected(key, issues),
        Ok(Plan::User { document, settings }) => {
            let saved = save_user(core.user_settings_path(), &document);
            let before = core.user_settings();
            core.replace_user_settings(*settings);
            apply_log_level(&before, &core.user_settings());
            // Open DevTools follow a changed `devtools.mode` at once (RB-010-06).
            crate::devtools::on_settings_changed(core);
            // Docked plugin panels follow `panels.dock` and `panels.dockSize` (RB-080-57).
            crate::panel_dock::on_settings_changed(core);
            if !saved {
                core.notify(CoreToShell::Notification(Notification {
                    level: NotificationLevel::Warning,
                    message_key: NOT_SAVED_KEY.to_owned(),
                    args: std::collections::BTreeMap::new(),
                    settings_key: Some(key.to_owned()),
                }));
            }
            changed(core, vec![key.to_owned()]);
            CoreToShell::Ack
        }
        Ok(Plan::Project { file, document }) => {
            let Some(root) = documents.project.map(|project| project.root) else {
                return rejected(key, write::refused(reason::NO_PROJECT));
            };
            let path = file.path(&root);
            if !writable(&path) {
                tracing::warn!(path = %path.display(), "a config file that cannot be read is not written over");
                return rejected(key, write::refused(reason::NOT_SAVED));
            }
            if let Err(error) = rb_config::write_config_file(&root, file, &document) {
                tracing::warn!(%error, "a setting could not be written");
                return rejected(key, write::refused(reason::NOT_SAVED));
            }
            // The watcher reloads the config and the grid follows (`layout.gap` at once).
            changed(core, vec![key.to_owned()]);
            CoreToShell::Ack
        }
    }
}

/// The shell's `setSetting` (RB-080-27): [`set_setting`] on a blocking thread of the runtime,
/// after every change before it, and `reply` with its answer from there - an internal failure
/// when the write panicked. The query comes in on the CEF UI thread (`cef_app/queries.rs`,
/// `ipc::dispatch_to`), which reads and writes no file this way.
pub fn set_setting_off_thread(
    core: &Core,
    key: String,
    value: Value,
    scope: Scope,
    reply: impl FnOnce(Result<CoreToShell, Failure>) + Send + 'static,
) {
    let Some(core) = core.arc() else {
        reply(Err((
            rb_ipc::IpcErrorCode::Internal,
            "the window is closed".to_owned(),
        )));
        return;
    };
    let runtime = core.runtime().clone();
    lane::reply_in_order(
        &runtime,
        move || set_setting(&core, &key, &value, scope, Caller::Shell),
        reply,
    );
}

/// The project config was read again: every page reads the project's keys again.
pub fn project_changed(core: &Core) {
    let keys = core
        .settings_registry()
        .iter()
        .filter(|def| !def.scope_allowed.contains(&Scope::User))
        .map(|def| def.key.clone())
        .collect();
    changed(core, keys);
}

/// Shows the native dialog; its choice follows as the event `pickedPath`.
pub fn pick_path(core: &Core, kind: PathKind, title_key: Option<&str>) {
    let title_key = title_key
        .filter(|key| rb_i18n::is_well_formed_key(key))
        .unwrap_or(PICK_TITLE_KEY);
    let me = core.arc();
    let pick = PathPick {
        kind,
        title: rb_i18n::t(title_key),
        save_as: None,
    };
    core.ui().pick_path(
        pick,
        Box::new(move |path| {
            if let Some(core) = me {
                core.send(&CoreToShell::PickedPath {
                    path: path.map(|path| path.to_string_lossy().into_owned()),
                });
            }
        }),
    );
}

fn changed(core: &Core, keys: Vec<String>) {
    core.send(&CoreToShell::SettingsChanged { keys });
}

fn rejected(key: &str, issues: Vec<SettingIssue>) -> CoreToShell {
    CoreToShell::SettingRejected {
        key: key.to_owned(),
        issues,
    }
}

/// Writes the user settings file ([`user_file::write`]); `false` when it is kept as it is (it
/// exists but cannot be read, or a newer rBrowser wrote it) or the write failed. Without a path
/// nothing is written.
fn save_user(path: Option<&Path>, document: &Value) -> bool {
    let Some(path) = path else {
        return true;
    };
    match user_file::write(path, document) {
        Ok(()) => true,
        Err(error) => {
            tracing::warn!(%error, "the user settings were not saved");
            false
        }
    }
}

/// Applies a changed `logs.level` to the running logger.
fn apply_log_level(before: &UserSettings, after: &UserSettings) {
    let (old, new) = (log_settings(before).level, log_settings(after).level);
    if old == new {
        return;
    }
    match LOG.get() {
        Some(handle) => {
            if let Err(error) = handle.set_level(new) {
                tracing::warn!(%error, "the log level could not be changed");
            }
        }
        None => tracing::debug!("no running logger to change the level of"),
    }
}
