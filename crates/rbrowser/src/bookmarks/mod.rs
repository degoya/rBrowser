//! Project bookmarks (RB-070-06): `bookmarks[]` of the project config (`rb_config::bookmarks`),
//! opened from the toolbar list, the shell's quick pick, the command palette, a bookmark's own
//! shortcut or MCP (`navigate` with the URL `bookmarks_list` gives), and added with "Bookmark
//! this page" into `config.json` or `config.local.json`.
//!
//! A bookmark holds a path, not a host (RB-080-48): a page that lies under the environments of
//! the project is stored as its path (`bookmarks.followEnvironment`, on; the dialog's switch per
//! bookmark) and opens on whichever environment is in effect; `bookmarks.make_relative`
//! (`Leader P P`) turns the absolute ones that exist into paths, and says how many. A page of a
//! foreign host stays an absolute URL. Which path a URL has is decided in one place,
//! `rb_config::bookmark_paths`; the shell holds no copy of it.
//!
//! | Part | Where |
//! | --- | --- |
//! | Model, check, the URL in an environment, `resolve` for other features | `rb_config::bookmarks` |
//! | The path of a URL under the environments (`path_in`, `stored_path`) | `rb_config::bookmark_paths` |
//! | Commands `bookmarks.open` (`Leader P B`), `bookmarks.add` (`Leader P K`), `bookmarks.make_relative` (`Leader P P`) | [`defs`] |
//! | The bookmark `bookmarks.add` describes, appended to a document | [`edit`] |
//! | Absolute bookmarks turned into paths; the bookmarks the check refuses | [`relative`] |
//! | Writing either to the project files, both or neither | [`store`] |
//! | MCP: `bookmarks_list` (each with `relative` and its URL now); `bookmarks_make_relative` runs the command | [`mcp`] |
//! | The bookmarks' own shortcuts and their conflicts | [`keys`] |
//! | The toolbar list, the quick pick and the dialog | `ui/src/bookmarks/`, `ui/src/toolbar/BookmarksMenu.vue` |
//!
//! Opening navigates every view whose navigation is coupled - or only the active view while its
//! navigation is decoupled (`Leader S U`, `syncNavigation: false`).

pub mod defs;
pub mod edit;
pub mod keys;
pub mod mcp;
pub mod relative;
mod store;

use rb_commands::{Binding, CommandError, Platform, Shortcut, builtin_reserved};
use rb_engine::{Target, ViewHost, ViewId};
use rb_ipc::{BookmarkSummary, CoreToShell, Notification, NotificationLevel};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::control::Core;
use crate::host::Host;

pub use defs::{ADD, MAKE_RELATIVE, OPEN, definitions, is_own, register};
pub use keys::{BookmarkKey, CONFLICT_KEY, Conflict};

/// The arguments of `bookmarks.open`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OpenArgs {
    /// The bookmark.
    id: String,
    /// One view instead of the coupled ones.
    #[serde(default)]
    view: Option<ViewId>,
}

/// Runs `bookmarks.open`, `bookmarks.add` or `bookmarks.make_relative` (registered by
/// [`register`]). Without arguments (a
/// shortcut, the toolbar button) the shell opens its quick pick or its dialog and runs the
/// command again with them.
pub fn run(core: &Core, id: &str, args: &Value) -> Result<Value, CommandError> {
    match id {
        OPEN if args.get("id").is_none() => ask_shell(core, id),
        OPEN => open(core, &parse(args)?),
        ADD => {
            let args: edit::AddArgs = if args.is_null() {
                edit::AddArgs::default()
            } else {
                parse(args)?
            };
            if args.is_empty() {
                ask_shell(core, id)
            } else {
                store::add(core, &args)
            }
        }
        MAKE_RELATIVE => store::make_relative(core),
        other => Err(CommandError::UnknownCommand(other.to_owned())),
    }
}

fn ask_shell(core: &Core, id: &str) -> Result<Value, CommandError> {
    // The quick pick and the dialog sit in the toolbar strip: the shell takes the keyboard.
    core.ui().focus_shell();
    core.send(&CoreToShell::UiCommand { id: id.to_owned() });
    Ok(Value::Null)
}

/// Loads the bookmark in the views [`open_target`] names (or `args.view`); answers its URL and
/// whether it follows the environment.
fn open(core: &Core, args: &OpenArgs) -> Result<Value, CommandError> {
    let host = core.host();
    let config = host.config();
    let bookmark = config
        .bookmarks
        .iter()
        .find(|bookmark| bookmark.id == args.id)
        .ok_or_else(|| {
            CommandError::InvalidArgs(format!("no bookmark has the id `{}`", args.id))
        })?;
    let environment = host.shell_state().environment;
    let base = rb_config::bookmarks::base_url(&config, environment.as_deref());
    let url = bookmark.url_with(base.as_deref()).ok_or_else(|| {
        CommandError::Failed(format!(
            "bookmark `{}` has no http or https URL here: a path needs an environment or start \
             URL to lie under, and must start with one `/`",
            bookmark.id
        ))
    })?;
    let target = args.view.map_or_else(|| open_target(host), Target::View);
    crate::nav_sync::navigate(host, target, &url)
        .map_err(|error| CommandError::Failed(error.to_string()))?;
    Ok(json!({ "id": bookmark.id, "url": url, "relative": bookmark.is_relative() }))
}

/// Where opening a bookmark navigates: the active view alone while its navigation is
/// decoupled, else every coupled view.
pub fn open_target(host: &Host) -> Target {
    match (host.active_view(), crate::nav_sync::for_host(host)) {
        (Some(active), Some(nav)) if !nav.is_coupled(active) => Target::View(active),
        _ => Target::All,
    }
}

/// The bookmarks as the shell lists them, with their URLs in the current environment.
pub fn summaries(host: &Host) -> Vec<BookmarkSummary> {
    let config = host.config();
    let environment = host.shell_state().environment;
    let base = rb_config::bookmarks::base_url(&config, environment.as_deref());
    config
        .bookmarks
        .iter()
        .map(|bookmark| BookmarkSummary {
            id: bookmark.id.clone(),
            label: bookmark.label.clone(),
            group: bookmark.group.clone(),
            url: bookmark.url_with(base.as_deref()),
            shortcut: bookmark.shortcut.clone(),
            path: bookmark.path.clone().filter(|_| bookmark.is_relative()),
        })
        .collect()
}

/// The core is up: the bookmarks' shortcuts are checked and their conflicts reported. Needs no
/// runtime (CEF calls it on its UI thread).
pub fn on_start(core: &Core) {
    report_conflicts(core);
}

/// The config was read again: the shortcuts are checked again.
pub fn on_config_changed(core: &Core) {
    report_conflicts(core);
}

/// The bookmark shortcuts in force and their conflicts, against the registry's bindings and the
/// breakpoints' own shortcuts.
pub fn shortcuts(core: &Core) -> (Vec<BookmarkKey>, Vec<Conflict>) {
    let CoreToShell::Commands { commands, .. } = core.commands() else {
        return (Vec::new(), Vec::new());
    };
    let mut held: Vec<(String, Binding)> = commands
        .iter()
        .flat_map(|command| {
            command
                .bindings
                .iter()
                .filter_map(|text| text.parse().ok())
                .map(|binding| (command.id.clone(), binding))
        })
        .collect();
    held.extend(
        crate::breakpoint_keys::project_shortcuts(core)
            .0
            .into_iter()
            .map(|key| {
                (
                    format!("breakpoint {}", key.breakpoint),
                    Binding::Direct(key.shortcut),
                )
            }),
    );
    keys::bookmark_keys(
        &core.host().config(),
        &held,
        builtin_reserved(),
        Platform::current(),
    )
}

fn report_conflicts(core: &Core) {
    for conflict in shortcuts(core).1 {
        tracing::warn!(
            bookmark = %conflict.bookmark,
            binding = %conflict.binding,
            reason = %conflict.reason_key,
            "a bookmark shortcut is not in force"
        );
        let args = std::collections::BTreeMap::from([
            ("bookmark".to_owned(), conflict.bookmark),
            ("binding".to_owned(), conflict.binding),
            ("reason".to_owned(), conflict.reason_key),
            ("other".to_owned(), conflict.other.unwrap_or_default()),
        ]);
        core.notify(CoreToShell::Notification(Notification {
            level: NotificationLevel::Warning,
            message_key: CONFLICT_KEY.to_owned(),
            args,
            settings_key: Some(rb_config::bookmarks::BOOKMARKS_KEY.to_owned()),
        }));
    }
}

/// A key no command and no breakpoint took: a bookmark's own shortcut opens it, off the calling
/// (UI) thread. A reserved combination is never taken. `true` when the key was taken.
pub fn on_unbound_key(core: &Core, shortcut: Shortcut) -> bool {
    let m = shortcut.modifiers;
    let may_be = m.ctrl || m.alt || m.meta || matches!(shortcut.key, rb_commands::Key::F(_));
    if !may_be
        || builtin_reserved()
            .find(&shortcut, Platform::current())
            .is_some()
    {
        return false;
    }
    let (keys, _) = shortcuts(core);
    let Some(id) = keys::bookmark_for(&keys, &shortcut) else {
        return false;
    };
    core.run_command_async(OPEN, json!({ "id": id }), |result| {
        if let Err(error) = result {
            tracing::warn!(%error, "a bookmark shortcut failed");
        }
    });
    true
}

fn parse<T: serde::de::DeserializeOwned>(args: &Value) -> Result<T, CommandError> {
    serde_json::from_value(args.clone())
        .map_err(|error| CommandError::InvalidArgs(error.to_string()))
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
