//! `bookmarks.add` on disk (RB-070-06): the bookmark appended to `config.json` or
//! `config.local.json`, held to the config's bookmark check and written in the canonical form
//! (`rb_config::write_config_file`: key order kept, so the file stays as it was apart from the
//! new entry). The config is read again right after the write, so the toolbar's list follows at
//! once ([`reload`]); the watcher reads it again too, where it hears the change at all.
//!
//! `bookmarks.make_relative` (RB-080-48) goes the same way with both files: the bookmarks
//! `super::relative` converts, written where something changed - both or neither
//! ([`write_all`]) -, read again, and the number said in the status bar.
//!
//! Both check only what they write. A bookmark in a file that the check refuses (one written
//! before a rule was tightened, or by hand) is left as it is - the loader drops it - and named
//! in a warning ([`INVALID_KEY`]); it does not stand in the way of the others.

use std::path::Path;

use rb_commands::CommandError;
use rb_config::ConfigFile;
use rb_engine::ViewHost;
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use serde_json::{Value, json};

use super::edit::{AddArgs, AddScope, Follow, Page, append, bookmark_of, ids_in};
use super::relative::{convert, invalid_ids};
use crate::control::Core;
use crate::settings_window::user_file;

/// Adds the bookmark `args` describe to the file of `args.scope`; answers its id, scope and URL
/// in the current environment.
pub fn add(core: &Core, args: &AddArgs) -> Result<Value, CommandError> {
    let host = core.host();
    let root = host
        .project_root()
        .ok_or_else(|| CommandError::Failed("no project is open".to_owned()))?;
    let (file, other) = match args.scope {
        AddScope::Project => (ConfigFile::Project, ConfigFile::Local),
        AddScope::Local => (ConfigFile::Local, ConfigFile::Project),
    };
    let path = file.path(&root);
    if !user_file::writable(&path) {
        return Err(CommandError::Failed(format!(
            "{} cannot be read, so it is not written over",
            file.file_name()
        )));
    }
    let mut document = read_or_empty(&path)?;
    let other_document = read_or_empty(&other.path(&root))?;
    let config = host.config();
    let mut taken = ids_in(&document);
    taken.extend(ids_in(&other_document));
    taken.extend(config.bookmarks.iter().map(|bookmark| bookmark.id.clone()));
    let environment = host.shell_state().environment;
    let environment_url = rb_config::bookmarks::base_url(&config, environment.as_deref());
    let path_of =
        |url: &str| rb_config::bookmark_paths::path_in(&config, environment.as_deref(), url);
    let follow = Follow {
        default: core.user_settings().bookmarks.follow_environment,
        path_of: &path_of,
        base: environment_url.as_deref(),
    };
    let bookmark = bookmark_of(args, active_page(core).as_ref(), &follow, &taken)
        .map_err(CommandError::InvalidArgs)?;
    checked(&bookmark)?;
    append(&mut document, &bookmark).map_err(CommandError::InvalidArgs)?;
    rb_config::write_config_file(&root, file, &document)
        .map_err(|error| CommandError::Failed(error.to_string()))?;
    reload(core, &root)?;
    if args.follow == Some(true) && !bookmark.is_relative() {
        // The dialog's switch was on, but the page belongs to no environment.
        notify(core, NotificationLevel::Info, KEPT_ABSOLUTE_KEY, &[]);
    }
    let invalid = warn_invalid(core, &[&document, &other_document]);
    let mut answer = json!({
        "added": bookmark.id,
        "scope": args.scope,
        "url": bookmark.url_with(environment_url.as_deref()),
        "relative": bookmark.is_relative(),
    });
    if !invalid.is_empty() {
        answer["invalid"] = json!(invalid);
    }
    Ok(answer)
}

/// The i18n key of the status bar's message after `bookmarks.make_relative`; placeholder
/// `{count}`.
pub const MADE_RELATIVE_KEY: &str = "bookmarks.make_relative.done";
/// The same when no bookmark was an absolute URL of an environment.
pub const NONE_RELATIVE_KEY: &str = "bookmarks.make_relative.none";
/// The message when a bookmark asked to follow the environment keeps its URL: the page belongs
/// to no environment.
pub const KEPT_ABSOLUTE_KEY: &str = "bookmarks.add.absolute";
/// The warning that names the bookmarks the check refuses; placeholder `{ids}`.
pub const INVALID_KEY: &str = "bookmarks.invalid.notice";

/// Turns every absolute bookmark of `config.json` and `config.local.json` that has a path
/// under the environments (`rb_config::bookmark_paths::stored_path`: it leads where it led)
/// into that path; answers how many (`converted`), which (`ids`) and the bookmarks the check
/// refuses (`invalid`, left as they are), and says the number in the status bar. A file
/// nothing changed in is not written; both files are written or neither.
pub fn make_relative(core: &Core) -> Result<Value, CommandError> {
    let host = core.host();
    let root = host
        .project_root()
        .ok_or_else(|| CommandError::Failed("no project is open".to_owned()))?;
    let mut project = read_or_empty(&ConfigFile::Project.path(&root))?;
    let mut local = read_or_empty(&ConfigFile::Local.path(&root))?;
    let config = host.config();
    let converted = convert(&mut project, &mut local, |url| {
        rb_config::bookmark_paths::stored_path(&config, url)
    });
    let changed: Vec<(ConfigFile, &Value)> = [
        (ConfigFile::Project, &project, converted.project),
        (ConfigFile::Local, &local, converted.local),
    ]
    .into_iter()
    .filter(|(_, _, changed)| *changed)
    .map(|(file, document, _)| (file, document))
    .collect();
    if !changed.is_empty() {
        if let Some((file, _)) = changed
            .iter()
            .find(|(file, _)| !user_file::writable(&file.path(&root)))
        {
            return Err(CommandError::Failed(format!(
                "{} cannot be read, so it is not written over",
                file.file_name()
            )));
        }
        write_all(&root, &changed, &|file, document| {
            rb_config::write_config_file(&root, file, document).map_err(|error| error.to_string())
        })?;
        reload(core, &root)?;
    }
    let ids = converted.ids;
    let count = ids.len().to_string();
    if ids.is_empty() {
        notify(
            core,
            NotificationLevel::Info,
            NONE_RELATIVE_KEY,
            &[("count", &count)],
        );
    } else {
        notify(
            core,
            NotificationLevel::Success,
            MADE_RELATIVE_KEY,
            &[("count", &count)],
        );
    }
    let invalid = warn_invalid(core, &[&project, &local]);
    let mut answer = json!({ "converted": ids.len(), "ids": ids });
    if !invalid.is_empty() {
        answer["invalid"] = json!(invalid);
    }
    Ok(answer)
}

/// Writes every file of `changed` with `write`, or none: when a write fails, the files written
/// before it - and the one that failed, which may be half written - get back the bytes they had
/// (a file that was not there is removed). One file with the new `path` beside the other with
/// the old `url` would be a bookmark with both, which the loader drops.
pub(super) fn write_all(
    root: &Path,
    changed: &[(ConfigFile, &Value)],
    write: &dyn Fn(ConfigFile, &Value) -> Result<(), String>,
) -> Result<(), CommandError> {
    let mut touched: Vec<(std::path::PathBuf, Option<Vec<u8>>)> = Vec::new();
    for (file, document) in changed {
        let path = file.path(root);
        touched.push((path.clone(), std::fs::read(&path).ok()));
        let Err(error) = write(*file, document) else {
            continue;
        };
        let lost: Vec<String> = touched
            .iter()
            .rev()
            .filter_map(|(path, before)| {
                let restored = match before {
                    Some(bytes) => std::fs::write(path, bytes),
                    None if path.exists() => std::fs::remove_file(path),
                    None => Ok(()),
                };
                restored.err().map(|_| path.display().to_string())
            })
            .collect();
        let state = if lost.is_empty() {
            "the config files are as they were".to_owned()
        } else {
            format!("could not be put back: {}", lost.join(", "))
        };
        return Err(CommandError::Failed(format!(
            "{} was not written ({error}); {state}",
            file.file_name()
        )));
    }
    Ok(())
}

/// Holds the bookmark a command writes to the config's bookmark check; what else the files
/// hold is not this command's.
fn checked(bookmark: &rb_config::Bookmark) -> Result<(), CommandError> {
    let alone = rb_config::ProjectConfig {
        bookmarks: vec![bookmark.clone()],
        ..rb_config::ProjectConfig::default()
    };
    let problems: Vec<String> = rb_config::bookmarks::check_bookmarks(&alone)
        .iter()
        .map(|issue| format!("{} {}", issue.path, issue.key))
        .collect();
    if problems.is_empty() {
        Ok(())
    } else {
        Err(CommandError::InvalidArgs(format!(
            "the bookmark does not pass the config check: {}",
            problems.join("; ")
        )))
    }
}

/// Names the bookmarks of `documents` the check refuses in a warning; answers their ids.
fn warn_invalid(core: &Core, documents: &[&Value]) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for id in documents.iter().flat_map(|document| invalid_ids(document)) {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    if !ids.is_empty() {
        tracing::warn!(bookmarks = %ids.join(", "), "bookmarks the config check refuses are left as they are");
        notify(
            core,
            NotificationLevel::Warning,
            INVALID_KEY,
            &[("ids", &ids.join(", "))],
        );
    }
    ids
}

fn notify(core: &Core, level: NotificationLevel, key: &str, args: &[(&str, &str)]) {
    core.notify(CoreToShell::Notification(Notification {
        level,
        message_key: key.to_owned(),
        args: args
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
        settings_key: Some(rb_config::bookmarks::BOOKMARKS_KEY.to_owned()),
    }));
}

/// Reads the config again and puts it in effect, so the list the shell shows holds the new
/// bookmark without a restart. The watcher does the same - but not over a WSL share
/// (`\\wsl.localhost`, which sends no change notifications), where the list waited for the
/// next start (RB-080-16). With nothing but the bookmarks changed the grid is left alone;
/// otherwise the config is applied as the watcher applies it.
fn reload(core: &Core, root: &Path) -> Result<(), CommandError> {
    let host = core.host();
    let loaded =
        crate::project::load(root).map_err(|error| CommandError::Failed(error.to_string()))?;
    let in_effect = host.config();
    let only_bookmarks = rb_config::ProjectConfig {
        bookmarks: loaded.bookmarks.clone(),
        ..in_effect
    } == loaded;
    if only_bookmarks {
        host.replace_config(loaded);
        super::on_config_changed(core);
    } else {
        host.open_config(loaded, Some(root.to_path_buf()))
            .map_err(|error| CommandError::Failed(error.to_string()))?;
        crate::features::on_config_changed(core);
    }
    Ok(())
}

/// The page of the active view (else the first view).
fn active_page(core: &Core) -> Option<Page> {
    let host = core.host();
    let views = host.views();
    host.active_view()
        .and_then(|active| views.iter().find(|view| view.id == active))
        .or_else(|| views.first())
        .map(|view| Page {
            url: view.url.clone(),
            title: view.title.clone(),
        })
}

/// The JSON of `path`; an empty object when there is no such file.
fn read_or_empty(path: &Path) -> Result<Value, CommandError> {
    if !path.is_file() {
        return Ok(json!({}));
    }
    rb_config::read_json(path).map_err(|error| CommandError::Failed(error.to_string()))
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
