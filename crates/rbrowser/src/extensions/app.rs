//! The extensions in the running app: the notice about folders the start did not load, a
//! trusted project's offer, adding a folder to the person's list (the settings window's write,
//! so the file, the settings in effect and every open settings page follow), and the state
//! `extension_list` answers. Nothing here installs without the person's own command.

use std::sync::Mutex;

use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use rb_settings::Scope;
use serde_json::{Value, json};

use super::folder::{FolderError, check};
use super::recommend::{Offer, fingerprint, offers, should_offer, with_folder};
use super::start::{self, StartPlan, needs_restart};
use super::web;
use crate::cdp::lock;
use crate::control::Core;
use crate::settings_window::{Caller, set_setting};

/// The settings key of the person's folders, which the notices link to.
pub const UNPACKED_KEY: &str = "engines.chromium.extensions.unpacked";
/// The notice: a trusted project recommends folders.
pub const RECOMMENDED_KEY: &str = "extensions.notice.recommended";
/// The notice: a trusted project recommends Web Store extensions only.
pub const STORE_ONLY_KEY: &str = "extensions.notice.store_only";
/// The notice: folders the start did not load.
pub const SKIPPED_KEY: &str = "extensions.notice.skipped";
/// The notice: a folder was added.
pub const ADDED_KEY: &str = "extensions.notice.added";
/// The notice: a trusted project's folders were added.
pub const ADDED_RECOMMENDED_KEY: &str = "extensions.notice.added_recommended";
/// The notice: some of a trusted project's recommendations were added, some failed.
pub const ADDED_RECOMMENDED_PARTLY_KEY: &str = "extensions.notice.added_recommended_partly";
/// The notice: a folder was refused.
pub const REFUSED_KEY: &str = "extensions.notice.refused";
/// The notice: the project offers nothing to add.
pub const NOTHING_KEY: &str = "extensions.notice.nothing_recommended";

/// The project and offers the last notice was shown for.
static LAST_OFFER: Mutex<Option<String>> = Mutex::new(None);

/// A notification with the person's folders linked.
pub fn notice(level: NotificationLevel, key: &str, args: &[(&str, String)]) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level,
        message_key: key.to_owned(),
        args: args
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect(),
        settings_key: Some(UNPACKED_KEY.to_owned()),
    })
}

/// The core is up: the folders the start refused are named once, then the project's offer.
pub fn on_start(core: &Core) {
    let started = start::started();
    if !started.skipped.is_empty() {
        let list = started
            .skipped
            .iter()
            .map(|skipped| format!("{} ({})", skipped.path, skipped.error))
            .collect::<Vec<_>>()
            .join("; ");
        core.notify(notice(
            NotificationLevel::Warning,
            SKIPPED_KEY,
            &[("count", started.skipped.len().to_string()), ("list", list)],
        ));
    }
    offer(core);
}

/// The config or the trust changed: the offer again, shown only when it changed.
pub fn on_config_changed(core: &Core) {
    offer(core);
}

/// The project's offers now; empty unless it may offer anything.
fn current_offers(core: &Core) -> Option<(std::path::PathBuf, Vec<Offer>)> {
    let host = core.host();
    let root = host.project_root()?;
    let settings = core.user_settings().engines.chromium.extensions;
    if !should_offer(host.shell_state().trusted, &settings) {
        return None;
    }
    let found = offers(
        &root,
        &host.config().extensions,
        &settings,
        &web::installed_ids(),
    );
    Some((root, found))
}

fn offer(core: &Core) {
    let Some((root, found)) = current_offers(core).filter(|(_, found)| !found.is_empty()) else {
        // Untrusted, switched off or nothing to offer: a later offer is shown again.
        *lock(&LAST_OFFER) = None;
        return;
    };
    let print = fingerprint(&root, &found);
    if lock(&LAST_OFFER).replace(print.clone()).as_deref() == Some(print.as_str()) {
        return;
    }
    let names = |pick: fn(&Offer) -> Option<&str>| -> Vec<String> {
        found.iter().filter_map(pick).map(str::to_owned).collect()
    };
    let folders = names(|offer| match offer {
        Offer::Folder { name, .. } => Some(name),
        _ => None,
    });
    let store = names(|offer| match offer {
        Offer::Store { written, .. } => Some(written),
        _ => None,
    });
    for refused in found.iter().filter_map(|offer| match offer {
        Offer::Refused { written, error } => Some((written, error)),
        _ => None,
    }) {
        tracing::warn!(path = %refused.0, code = refused.1.code(), "a recommended extension is refused: {}", refused.1);
    }
    if !folders.is_empty() {
        core.notify(notice(
            NotificationLevel::Info,
            RECOMMENDED_KEY,
            &[
                ("count", folders.len().to_string()),
                ("names", folders.join(", ")),
            ],
        ));
    } else if !store.is_empty() {
        core.notify(notice(
            NotificationLevel::Info,
            STORE_ONLY_KEY,
            &[("names", store.join(", "))],
        ));
    }
}

/// Adds the folder at `path` to the person's list (or switches its entry back on); answers the
/// extension's name. The caller is the person's command, never MCP.
///
/// # Errors
///
/// The folder is no extension rBrowser loads, or the settings refused the write.
pub fn add_folder(core: &Core, path: &str) -> Result<String, String> {
    let found = check(path).map_err(|error: FolderError| error.to_string())?;
    let folder = found.folder.to_string_lossy().into_owned();
    let current = core.user_settings().engines.chromium.extensions.unpacked;
    let list =
        serde_json::to_value(with_folder(&current, &folder)).map_err(|error| error.to_string())?;
    match set_setting(core, UNPACKED_KEY, &list, Scope::User, Caller::Shell) {
        CoreToShell::Ack => Ok(found.manifest.name),
        CoreToShell::SettingRejected { issues, .. } => Err(issues
            .iter()
            .map(|issue| rb_i18n::t(&issue.message_key))
            .collect::<Vec<_>>()
            .join("; ")),
        other => Err(format!("unexpected answer {other:?}")),
    }
}

/// What adding a project's recommendations did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Recommended {
    /// Folders added and Web Store extensions installed.
    pub added: usize,
    /// What failed: the entry as written and why.
    pub failed: Vec<(String, String)>,
}

/// Adds every valid folder the trusted project recommends and installs its Web Store entries
/// that name an id (while the Web Store is allowed; RB-080-23). A failure stops nothing after
/// it: every entry is tried, the failures counted with their reasons.
///
/// # Errors
///
/// The project is not trusted (or offers are off).
pub fn add_recommended(core: &Core) -> Result<Recommended, String> {
    let (_, found) = current_offers(core).ok_or_else(|| {
        "the project's recommendations are offered only in a trusted workspace with \
         engines.chromium.extensions.enabled and offerProjectRecommendations on"
            .to_owned()
    })?;
    let allow = core
        .user_settings()
        .engines
        .chromium
        .extensions
        .allow_web_store;
    Ok(add_each(
        &found,
        allow,
        |folder| add_folder(core, folder).map(|_| ()),
        |id| web::install_by_ref(core, id).map(|_| ()),
    ))
}

/// [`add_recommended`] over its inputs: `add` takes a folder, `install` a Web Store id (only
/// while `allow`); a store entry without an id and a refused path are left out.
pub fn add_each(
    offers: &[Offer],
    allow: bool,
    mut add: impl FnMut(&str) -> Result<(), String>,
    mut install: impl FnMut(&str) -> Result<(), String>,
) -> Recommended {
    let mut done = Recommended::default();
    for offer in offers {
        let (written, result) = match offer {
            Offer::Folder {
                written, folder, ..
            } => (written, add(&folder.to_string_lossy())),
            Offer::Store {
                written,
                id: Some(id),
            } if allow => (written, install(id)),
            Offer::Store { .. } | Offer::Refused { .. } => continue,
        };
        match result {
            Ok(()) => done.added += 1,
            Err(error) => {
                tracing::warn!(entry = %written, "a recommended extension was not added: {error}");
                done.failed.push((written.clone(), error));
            }
        }
    }
    done
}

/// The notice after [`add_recommended`]: nothing to add, all added, or the counts with the
/// first failure's reason.
pub fn recommended_notice(done: &Recommended) -> CoreToShell {
    match (done.added, done.failed.first()) {
        (0, None) => notice(NotificationLevel::Info, NOTHING_KEY, &[]),
        (added, None) => notice(
            NotificationLevel::Info,
            ADDED_RECOMMENDED_KEY,
            &[("count", added.to_string())],
        ),
        (added, Some((written, error))) => notice(
            if added == 0 {
                NotificationLevel::Error
            } else {
                NotificationLevel::Warning
            },
            ADDED_RECOMMENDED_PARTLY_KEY,
            &[
                ("added", added.to_string()),
                ("failed", done.failed.len().to_string()),
                ("first", format!("{written}: {error}")),
            ],
        ),
    }
}

/// What `extension_list` answers: the switches, every folder of the list and every installed
/// Web Store extension with whether this start loaded it (or why not), whether a restart is
/// needed, and a trusted project's offers.
pub fn status(core: &Core) -> Value {
    let settings = core.user_settings().engines.chromium.extensions;
    let offered = current_offers(core).map(|(_, found)| found);
    status_answer(
        &settings,
        &start::started(),
        &start::current_plan(&settings),
        offered.as_deref(),
    )
}

/// [`status`] over its inputs.
pub fn status_answer(
    settings: &rb_config::ChromiumExtensionSettings,
    started: &StartPlan,
    now: &StartPlan,
    offered: Option<&[Offer]>,
) -> Value {
    let loaded = started.loaded_paths();
    let mut extensions: Vec<Value> = settings
        .unpacked
        .iter()
        .map(|entry| {
            let mut row = json!({
                "path": entry.path,
                "enabled": entry.enabled,
                "loaded": false,
            });
            match check(&entry.path) {
                Ok(found) => {
                    let folder = found.folder.to_string_lossy();
                    row["loaded"] =
                        json!(loaded.iter().any(|path| path.eq_ignore_ascii_case(&folder)));
                    row["name"] = json!(found.manifest.name);
                    row["version"] = json!(found.manifest.version);
                    if let Some(id) = found.manifest.id {
                        row["id"] = json!(id);
                    }
                }
                Err(error) => {
                    row["error"] = json!({ "code": error.code(), "message": error.to_string() });
                }
            }
            row
        })
        .collect();
    extensions.extend(now.web_store.iter().map(|installed| {
        let folder = installed.folder.to_string_lossy();
        json!({
            "source": "webStore",
            "id": installed.id,
            "name": installed.name,
            "version": installed.version,
            "path": folder,
            "enabled": settings.enabled,
            "loaded": loaded.iter().any(|path| path.eq_ignore_ascii_case(&folder)),
        })
    }));
    let recommendations: Vec<Value> = offered
        .unwrap_or_default()
        .iter()
        .map(|offer| match offer {
            Offer::Folder { written, name, .. } => json!({ "path": written, "name": name }),
            Offer::Store { written, id } => {
                let mut row = json!({
                    "store": written,
                    "installable": settings.allow_web_store && id.is_some(),
                });
                if let Some(id) = id {
                    row["id"] = json!(id);
                }
                row
            }
            Offer::Refused { written, error } => {
                json!({ "path": written, "error": { "code": error.code(), "message": error.to_string() } })
            }
        })
        .collect();
    json!({
        "enabled": settings.enabled,
        "restartNeeded": needs_restart(started, now),
        "extensions": extensions,
        "projectRecommendations": recommendations,
        "webStore": settings.allow_web_store,
        "checkUpdatesAtStart": settings.check_updates_at_start,
    })
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
