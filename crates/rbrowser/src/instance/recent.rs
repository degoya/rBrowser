//! The recent projects (General > recent projects, `recentProjects`): the open project moves to
//! the front when an instance starts, and `project.open_recent` starts rBrowser on one of them
//! (one project per instance, so it is a new process - which hands over to the project's own
//! instance when that runs already).

use std::path::Path;

use rb_commands::CommandError;
use rb_process::NoConsoleWindow as _;
use serde_json::{Map, Value};

use super::file;
use crate::control::Core;
use crate::settings_window::user_file::UserFileError;

/// The id of the command.
pub const OPEN_RECENT: &str = "project.open_recent";
/// The most projects the list keeps.
pub const MAX_RECENT: usize = 10;
/// The i18n key of the failure when no other recent project exists.
pub const NO_RECENT_KEY: &str = "project.error.noRecent";
/// The i18n key of the failure when the picked project has no config any more.
pub const MISSING_KEY: &str = "project.error.missing";
/// The i18n key of the failure when rBrowser cannot be started again.
pub const NOT_STARTED_KEY: &str = "project.error.notStarted";
/// The settings key of the list.
const RECENT_KEY: &str = "recentProjects";

/// `recent` with `root` in front, each project once (compared as the instance key does), at
/// most [`MAX_RECENT`].
pub fn push_recent(recent: &[String], root: &str) -> Vec<String> {
    let key = file::key(Path::new(root));
    std::iter::once(root.to_owned())
        .chain(
            recent
                .iter()
                .filter(|entry| file::key(Path::new(entry.as_str())) != key)
                .cloned(),
        )
        .take(MAX_RECENT)
        .collect()
}

/// The project `project.open_recent` opens: `{ "path": p }` (one of `recent`), `{ "index": n }`,
/// or without arguments the newest that is not `current`.
pub fn pick(
    recent: &[String],
    current: Option<&Path>,
    args: &Value,
) -> Result<String, CommandError> {
    let invalid = |why: &str| CommandError::InvalidArgs(why.to_owned());
    let empty = Map::new();
    let args = match args {
        Value::Null => &empty,
        Value::Object(args) => args,
        _ => return Err(invalid("expected an object")),
    };
    if let Some(name) = args
        .keys()
        .find(|name| !matches!(name.as_str(), "path" | "index"))
    {
        return Err(invalid(&format!("unknown argument `{name}`")));
    }
    if let Some(path) = args.get("path") {
        let path = path.as_str().ok_or_else(|| invalid("`path` is a string"))?;
        let key = file::key(Path::new(path));
        return recent
            .iter()
            .find(|entry| file::key(Path::new(entry.as_str())) == key)
            .cloned()
            .ok_or_else(|| invalid("`path` is not a recent project"));
    }
    if let Some(index) = args.get("index") {
        return index
            .as_u64()
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| recent.get(index))
            .cloned()
            .ok_or_else(|| invalid("`index` is not in the list"));
    }
    let current = current.map(file::key);
    recent
        .iter()
        .find(|entry| Some(file::key(Path::new(entry.as_str()))) != current)
        .cloned()
        .ok_or_else(|| CommandError::Failed(NO_RECENT_KEY.to_owned()))
}

/// Runs `project.open_recent`: starts rBrowser on the picked project.
pub fn open_recent(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let recent = core.user_settings().recent_projects;
    let current = core.host().project_root();
    let target = pick(&recent, current.as_deref(), args)?;
    let config = Path::new(&target)
        .join(rb_config::CONFIG_DIR)
        .join(rb_config::CONFIG_FILE);
    if !config.is_file() {
        return Err(CommandError::Failed(MISSING_KEY.to_owned()));
    }
    let not_started = |error: std::io::Error| {
        tracing::warn!(%error, "rBrowser could not be started for a recent project");
        CommandError::Failed(NOT_STARTED_KEY.to_owned())
    };
    let exe = std::env::current_exe().map_err(not_started)?;
    let mut child = std::process::Command::new(exe)
        .arg("--project")
        .arg(&target)
        .no_console_window()
        .spawn()
        .map_err(not_started)?;
    // The new process opens its window or hands over and ends; it is reaped here.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(serde_json::json!({ "path": target }))
}

/// Moves the project at `root` to the front of `recentProjects` and writes the list back -
/// unless the settings file did not load, which is never written over.
pub fn record(core: &Core, root: &Path) {
    let root = root.to_string_lossy();
    let user = core.user_settings();
    let list = push_recent(&user.recent_projects, &root);
    if list == user.recent_projects {
        return;
    }
    if let Some(path) = core.user_settings_path()
        && let Err(error) = set_recent(path, &list)
    {
        tracing::warn!(%error, "the recent projects were not saved");
        if matches!(error, UserFileError::Kept) {
            return;
        }
    }
    let mut user = user;
    user.recent_projects = list;
    core.replace_user_settings(user);
}

/// Sets `recentProjects` in the settings file at `path`, keeping every other key; a file that
/// does not read or comes from a newer rBrowser is left alone
/// ([`crate::settings_window::user_file::update`]).
pub fn set_recent(path: &Path, list: &[String]) -> Result<(), UserFileError> {
    crate::settings_window::user_file::update(path, |map| {
        map.insert(RECENT_KEY.to_owned(), Value::from(list.to_vec()));
    })
}

#[cfg(test)]
#[path = "recent_tests.rs"]
mod tests;
