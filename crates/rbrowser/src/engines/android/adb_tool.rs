//! The Android Platform Tools as a managed tool (RB-095-02, the owner's decision of
//! 2026-11-21): when no installed `adb` is found, rBrowser offers to download Google's
//! Platform Tools - pinned address, SHA-256 and size in `crate::tools` - and does so only on the
//! person's command `tools.adb.install` ([`install_default`]); no MCP tool reaches it. They are
//! never shipped: the Android SDK licence stays Google's to grant, and the package stays small.
//! The consent notice ([`needed_notice`]) names the version, size, source host and licence and
//! links `engines.android.adbPath`; nothing touches the network before.
//!
//! Google's terms (RB-099-13, the owner's decision of 2026-11-25): the command without
//! `{ "accepted": true }` downloads nothing - it asks the shell to show the terms of the Platform
//! Tools ([`TERMS_URL`]) with the version, size, source host and licence, and only the person's
//! "Accept and download" there runs it again with the acceptance ([`step`]).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::cdp::lock;
use crate::tools::fetch::{Fetcher, HttpFetcher};
use crate::tools::manifest::megabytes;
use crate::tools::{Installed, ToolEntry, ToolError, ToolStore};

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
/// Google's page of the Platform Tools with the terms their download is under (the Android
/// Software Development Kit License Agreement), shown before the download.
pub const TERMS_URL: &str = "https://developer.android.com/tools/releases/platform-tools";

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

/// `tools.adb.install`'s arguments.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstallArgs {
    /// The person accepted Google's terms in the shell's terms bar.
    #[serde(default)]
    accepted: bool,
}

/// What `tools.adb.install` does next (boxed: the variants stay small).
#[derive(Debug)]
pub enum Step {
    /// Not accepted yet: the shell's terms bar (a `uiCommand`); nothing is downloaded.
    AskTerms(Box<CoreToShell>),
    /// Accepted: download this entry.
    Download(Box<ToolEntry>),
    /// No build to offer: the failure notice.
    Refused(Box<CoreToShell>),
}

/// The step of `tools.adb.install` with `args` for the manifest's `entry`.
///
/// # Errors
///
/// Arguments that do not read (`accepted` no boolean, an unknown field).
pub fn step(args: &Value, entry: Result<ToolEntry, ToolError>) -> Result<Step, String> {
    let args: InstallArgs = if args.is_null() {
        InstallArgs::default()
    } else {
        serde_json::from_value(args.clone()).map_err(|error| error.to_string())?
    };
    Ok(match entry {
        Err(error) => Step::Refused(Box::new(failed(&error))),
        Ok(entry) if args.accepted => Step::Download(Box::new(entry)),
        Ok(entry) => Step::AskTerms(Box::new(terms_request(&entry))),
    })
}

/// The shell's terms bar for `entry`: what would be downloaded, from where, under which terms.
fn terms_request(entry: &ToolEntry) -> CoreToShell {
    let mut args: serde_json::Map<String, Value> = entry_args(entry)
        .into_iter()
        .map(|(name, value)| (name.to_owned(), Value::String(value)))
        .collect();
    args.insert("terms".to_owned(), json!(TERMS_URL));
    CoreToShell::UiCommand {
        id: super::commands::INSTALL.to_owned(),
        args: Some(Value::Object(args)),
    }
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

/// [`install`] of `entry` (the one [`step`] accepted) over https into the default store.
///
/// # Errors
///
/// No tools folder, or as [`install`].
pub fn install_default(
    entry: &ToolEntry,
    notify: &dyn Fn(CoreToShell),
) -> Result<Installed, ToolError> {
    match default_store() {
        Some(store) => install(&store, &HttpFetcher::new(), entry, notify),
        None => {
            let error = ToolError::Io("no folder for tools".to_owned());
            notify(failed(&error));
            Err(error)
        }
    }
}

#[cfg(test)]
#[path = "adb_tool_tests.rs"]
mod tests;
