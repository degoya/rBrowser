//! The Android Platform Tools as a managed tool (RB-095-02, the owner's decision of
//! 2026-11-21): when no installed `adb` is found, rBrowser offers to download Google's
//! Platform Tools - pinned address, SHA-256 and size in `crate::tools` - and does so only on the
//! person's command `tools.adb.install` ([`install_default`]); no MCP tool reaches it. They are
//! never shipped: the Android SDK licence stays Google's to grant, and the package stays small.
//! The consent notice ([`needed_notice`]) names the version, size, source host and licence and
//! links `engines.android.adbPath`; nothing touches the network before.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use rb_ipc::{CoreToShell, Notification, NotificationLevel};

use crate::cdp::lock;
use crate::tools::fetch::{Fetcher, HttpFetcher};
use crate::tools::manifest::megabytes;
use crate::tools::{Installed, ToolEntry, ToolError, ToolStore, adb_entry};

/// No adb; placeholders `{reason}`, `{version}`, `{size}`, `{license}`, `{from}`.
pub const NEEDED_KEY: &str = "engines.android.notice.adb_needed";
/// The download started; the same placeholders.
pub const DOWNLOADING_KEY: &str = "engines.android.notice.adb_downloading";
/// The download is done; `{version}`, `{folder}`.
pub const INSTALLED_KEY: &str = "engines.android.notice.adb_installed";
/// The download failed; `{error}`.
pub const FAILED_KEY: &str = "engines.android.notice.adb_failed";
/// A download runs already.
pub const BUSY_KEY: &str = "engines.android.notice.adb_busy";
/// The setting the adb notices link to.
pub const PATH_SETTING: &str = "engines.android.adbPath";

/// A download in progress.
static INSTALLING: Mutex<bool> = Mutex::new(false);

/// The downloaded adb of `entry` in `store`, when there is one.
pub fn managed(store: Option<&ToolStore>, entry: &ToolEntry) -> Option<PathBuf> {
    store?
        .installed(entry)
        .map(|installed| installed.executable)
}

/// The managed store at its default place.
pub fn default_store() -> Option<ToolStore> {
    ToolStore::default_root().map(ToolStore::new)
}

/// The consent notice: why adb is needed, and what the download would be.
pub fn needed_notice(entry: &ToolEntry, reason: &str) -> CoreToShell {
    let mut args = entry_args(entry);
    args.push(("reason", reason.to_owned()));
    notice(NotificationLevel::Warning, NEEDED_KEY, &args)
}

fn entry_args(entry: &ToolEntry) -> Vec<(&'static str, String)> {
    let from = entry
        .url
        .strip_prefix("https://")
        .and_then(|rest| rest.split('/').next())
        .unwrap_or_default()
        .to_owned();
    vec![
        ("version", entry.version.clone()),
        ("size", megabytes(entry.size)),
        ("license", entry.license.clone()),
        ("from", from),
    ]
}

/// A notification with the adb path setting linked.
pub fn notice(level: NotificationLevel, key: &str, args: &[(&str, String)]) -> CoreToShell {
    let args: BTreeMap<String, String> = args
        .iter()
        .map(|(name, value)| ((*name).to_owned(), value.clone()))
        .collect();
    CoreToShell::Notification(Notification {
        level,
        message_key: key.to_owned(),
        args,
        settings_key: Some(PATH_SETTING.to_owned()),
    })
}

/// Downloads and activates `entry` with `fetcher` into `store` (blocking), telling the person
/// through `notify`. One download at a time.
///
/// # Errors
///
/// A download runs already, or the install failed (nothing new is active then).
pub fn install(
    store: &ToolStore,
    fetcher: &dyn Fetcher,
    entry: &ToolEntry,
    notify: &dyn Fn(CoreToShell),
) -> Result<Installed, ToolError> {
    {
        let mut installing = lock(&INSTALLING);
        if *installing {
            notify(notice(NotificationLevel::Info, BUSY_KEY, &[]));
            return Err(ToolError::Busy {
                tool: entry.name.clone(),
            });
        }
        *installing = true;
    }
    notify(notice(
        NotificationLevel::Info,
        DOWNLOADING_KEY,
        &entry_args(entry),
    ));
    let result = store.install(fetcher, entry);
    *lock(&INSTALLING) = false;
    match &result {
        Ok(installed) => notify(notice(
            NotificationLevel::Info,
            INSTALLED_KEY,
            &[
                ("version", installed.version.clone()),
                ("folder", installed.dir.display().to_string()),
            ],
        )),
        Err(error) => notify(failed(error)),
    }
    result
}

fn failed(error: &ToolError) -> CoreToShell {
    tracing::warn!(%error, "the adb download failed");
    notice(
        NotificationLevel::Error,
        FAILED_KEY,
        &[("error", error.to_string())],
    )
}

/// [`install`] over https into the default store.
///
/// # Errors
///
/// No tools folder, no build for this platform, or as [`install`].
pub fn install_default(notify: &dyn Fn(CoreToShell)) -> Result<Installed, ToolError> {
    let prepared = default_store()
        .ok_or_else(|| ToolError::Io("no folder for tools".to_owned()))
        .and_then(|store| Ok((store, adb_entry()?)));
    match prepared {
        Ok((store, entry)) => install(&store, &HttpFetcher::new(), &entry, notify),
        Err(error) => {
            notify(failed(&error));
            Err(error)
        }
    }
}

#[cfg(test)]
#[path = "adb_tool_tests.rs"]
mod tests;
