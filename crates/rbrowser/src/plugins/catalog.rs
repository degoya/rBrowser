//! The plugin catalogue in the app (RB-091-01): Settings > Plugins > Catalog asks for it
//! (`pluginPanel` with `op` `catalog`, `catalogInstall`, `catalogRemove`), the commands
//! `plugins.catalog` and `plugins.check_updates` open it or check for updates, and MCP reads it
//! through `plugins_catalog_list`.
//!
//! The repositories are the official catalogue (its address and key built in) and the ones in
//! `plugins.catalog.repositories`, a protected setting only a person writes. Everything here
//! runs on a blocking thread: a refresh and a download reach the network
//! (`rb_plugin_host::HttpFetcher`, https only, bounded). **An install is a person's**: it comes
//! from the settings window after a review of its full grants and key (`catalog_install.rs`), never from MCP,
//! whose tool lists and refreshes only. A confirmed publisher key is pinned in
//! `plugins.trustedKeys` as the settings window would pin it, and the plugin loads at the next
//! start like any user-installed one.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::Utc;
use rb_config::UserSettings;
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use rb_plugin_host::{
    CATALOG_DIR, Catalog, CatalogStore, Fetcher, HttpFetcher, MAX_USER_REPOSITORIES,
    OFFICIAL_INDEX_URL, Repository, RepositoryError,
};
use serde_json::{Map, Value, json};

use crate::control::Core;

/// The status bar's notice of updates (`{count}`).
pub const UPDATES_NOTICE_KEY: &str = "plugins.catalog.notice.updates";
/// The status bar's notice that the catalogue has no update.
pub const NO_UPDATES_NOTICE_KEY: &str = "plugins.catalog.notice.no_updates";
/// The status bar's notice of updates installed by themselves (`{count}`,
/// `plugins.catalog.autoUpdate`, RB-091-04).
pub const AUTO_UPDATED_NOTICE_KEY: &str = "plugins.catalog.notice.auto_updated";
/// The status bar's notice that a check failed; the catalogue's tab names which repository.
pub const CHECK_FAILED_NOTICE_KEY: &str = "plugins.catalog.notice.check_failed";
/// The setting the notices link to: the catalogue's tab of Settings > Plugins.
pub const CATALOG_SETTING: &str = "plugins.catalog.repositories";

/// Whether this process ran its start check already.
static START_CHECKED: AtomicBool = AtomicBool::new(false);

#[path = "catalog_install.rs"]
mod install;

pub use install::{
    ConsentRequest, FOREIGN_CODE, InstallRequest, install, refuse_foreign, remove, review,
    same_grants, with_pinned,
};

/// The catalogue's folder beside the user settings.
pub fn store_dir() -> Option<PathBuf> {
    rb_config::user_settings_path().map(|settings| settings.with_file_name(CATALOG_DIR))
}

/// The repositories of `user`'s settings: the official catalogue first, then the person's in
/// their order - each address and key once, at most [`MAX_USER_REPOSITORIES`] - and, apart, the
/// entries that are not a repository (`{name, url, error}`, the error an i18n key).
pub fn repositories(user: &UserSettings) -> (Vec<Repository>, Vec<Value>) {
    let mut list = Vec::new();
    let mut refused = Vec::new();
    match Repository::official() {
        Ok(official) => list.push(official),
        Err(error) => refused.push(json!({
            "name": "rBrowser",
            "url": OFFICIAL_INDEX_URL,
            "error": error.code(),
        })),
    }
    for configured in user
        .plugins
        .catalog
        .repositories
        .iter()
        .take(MAX_USER_REPOSITORIES)
    {
        match Repository::user(&configured.name, &configured.url, &configured.public_key) {
            Ok(repository) if list.iter().all(|known| known.id != repository.id) => {
                list.push(repository);
            }
            Ok(_) => {}
            Err(error) => refused.push(json!({
                "name": configured.name,
                "url": without_query(&configured.url),
                "error": error.code(),
            })),
        }
    }
    (list, refused)
}

/// The catalogue of `user`'s repositories over `dir`, its cached indexes loaded.
pub fn open_catalog(user: &UserSettings, dir: PathBuf) -> (Catalog, Vec<Value>) {
    let (repositories, refused) = repositories(user);
    let mut catalog = Catalog::new(CatalogStore::new(dir), repositories);
    catalog.load_cached(Utc::now());
    (catalog, refused)
}

/// The catalogue page's answer: the repositories with their state, the offers, the updates and
/// the catalogue's installs; refreshed through `fetcher` first when there is one. `installed`
/// is what the start loaded (`{id: {version, source}}`, `null` while it loads).
pub fn answer(
    user: &UserSettings,
    dir: PathBuf,
    fetcher: Option<&dyn Fetcher>,
    installed: Value,
) -> Value {
    let (mut catalog, refused) = open_catalog(user, dir);
    let now = Utc::now();
    let outcomes = fetcher
        .map(|fetcher| catalog.refresh(fetcher, now))
        .unwrap_or_default();
    let repositories: Vec<Value> = catalog
        .repositories()
        .iter()
        .map(|repository| {
            let error = outcomes
                .iter()
                .find(|(id, _)| *id == repository.id)
                .and_then(|(_, outcome)| outcome.as_ref().err())
                .map(RepositoryError::code);
            json!({
                "id": repository.id,
                "name": repository.name,
                "url": without_query(repository.url.as_str()),
                "official": repository.is_official(),
                "fingerprint": repository.key_fingerprint(),
                "loaded": catalog.is_loaded(&repository.id, now),
                "error": error,
            })
        })
        .collect();
    json!({
        "repositories": repositories,
        "refused": refused,
        "entries": catalog.entries(now),
        "updates": catalog.updates(now),
        "installs": catalog.store().installs(),
        "installed": installed,
    })
}

/// What the start loaded, by plugin id: `{version, source}`; `null` while the load runs.
pub fn installed_now() -> Value {
    let Some(rows) = super::list::current() else {
        return Value::Null;
    };
    let mut installed = Map::new();
    for row in rows.into_iter().filter(|row| row.verified) {
        installed.insert(
            row.id,
            json!({ "version": row.version, "source": row.source }),
        );
    }
    Value::Object(installed)
}

/// An address without user name, password, query and fragment: what an answer names, so a
/// token someone put into a repository's address never reaches a page, a log or MCP (security
/// review L1).
pub fn without_query(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(mut parsed) => {
            parsed.set_query(None);
            parsed.set_fragment(None);
            // Neither fails for an address with a host, the only kind with credentials.
            let _ = parsed.set_username("");
            let _ = parsed.set_password(None);
            parsed.to_string()
        }
        Err(_) => {
            let bare = url.split(['?', '#']).next().unwrap_or_default();
            // Unparsed: whatever stands before an `@` of the authority is dropped.
            match bare.split_once("://") {
                Some((scheme, rest)) => match rest.split('/').next().and_then(|authority| {
                    authority
                        .rsplit_once('@')
                        .map(|(_, host)| (authority.len(), host))
                }) {
                    Some((length, host)) => format!("{scheme}://{host}{}", &rest[length..]),
                    None => bare.to_owned(),
                },
                None => bare.to_owned(),
            }
        }
    }
}

/// Refreshes the catalogue and says in the status bar how many updates there are: the command
/// `plugins.check_updates` from a shortcut or the palette, and the start check. Off the UI
/// thread.
pub fn check_in_background(core: &Core) {
    let Some(core) = core.arc() else {
        return;
    };
    let runtime = core.runtime().clone();
    runtime.spawn_blocking(move || {
        let Some(dir) = store_dir() else {
            return;
        };
        let user = core.user_settings();
        let mut answer = answer(&user, dir, Some(&HttpFetcher::new()), Value::Null);
        // RB-091-04: what may install by itself does, then the rest is counted with the
        // bundled plugins' updates.
        let updated = super::bundle_offer::auto_update(&core);
        if updated > 0 {
            core.notify(CoreToShell::Notification(Notification {
                level: NotificationLevel::Info,
                message_key: AUTO_UPDATED_NOTICE_KEY.to_owned(),
                args: [("count".to_owned(), updated.to_string())].into(),
                settings_key: Some(CATALOG_SETTING.to_owned()),
            }));
        }
        if let Value::Object(fields) = &mut answer {
            fields.insert(
                "bundled".to_owned(),
                Value::Array(super::bundle_offer::bundled_rows(&user)),
            );
        }
        core.notify(CoreToShell::Notification(notice(&answer)));
    });
}

/// The status bar's notice of a check's `answer`: the catalogue's updates and the bundled
/// plugins' (`bundled[].update`, RB-091-04).
pub fn notice(answer: &Value) -> Notification {
    let bundled = answer["bundled"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|row| row["update"].is_object())
        .count();
    let updates = answer["updates"].as_array().map_or(0, Vec::len) + bundled;
    // The page names the repository that failed and why; the notice says a sentence only.
    let failed = answer["repositories"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|repository| repository["error"].is_string());
    let (level, message_key, args) = match (updates, failed) {
        (0, true) => (
            NotificationLevel::Warning,
            CHECK_FAILED_NOTICE_KEY,
            Default::default(),
        ),
        (0, false) => (
            NotificationLevel::Info,
            NO_UPDATES_NOTICE_KEY,
            Default::default(),
        ),
        (count, _) => (
            NotificationLevel::Info,
            UPDATES_NOTICE_KEY,
            [("count".to_owned(), count.to_string())].into(),
        ),
    };
    Notification {
        level,
        message_key: message_key.to_owned(),
        args,
        settings_key: Some(CATALOG_SETTING.to_owned()),
    }
}

/// At the first start of the process: the update check, when `plugins.catalog.checkOnStart`
/// is on.
pub fn on_start(core: &Core) {
    if !core.user_settings().plugins.catalog.check_on_start
        || START_CHECKED.swap(true, Ordering::AcqRel)
    {
        return;
    }
    check_in_background(core);
}

/// MCP's `plugins_catalog_list`: the catalogue, refreshed first with `refresh`. It installs
/// nothing and adds no repository. Blocking.
pub fn for_mcp(core: &Core, refresh: bool) -> Value {
    let Some(dir) = store_dir() else {
        return json!({ "repositories": [], "entries": [], "updates": [] });
    };
    let fetcher = HttpFetcher::new();
    let fetcher: Option<&dyn Fetcher> = if refresh { Some(&fetcher) } else { None };
    let mut answer = answer(&core.user_settings(), dir, fetcher, installed_now());
    // The person's installs record nothing an agent needs beyond `installed`.
    if let Value::Object(fields) = &mut answer {
        fields.remove("installs");
    }
    answer
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
