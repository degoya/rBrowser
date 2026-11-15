//! Which ffmpeg records, and getting the managed one.
//!
//! [`locate`]: the user's own (`tools.ffmpegPath`, user settings only) when set, else the
//! managed build under `%LOCALAPPDATA%\rBrowser\tools\ffmpeg\<version>\` - never a path a
//! project names. [`probe`] reads `ffmpeg -version` once per program: the managed build must
//! be an LGPL build (`--enable-gpl` and `--enable-nonfree` absent); the user's own is the
//! user's choice and only noted in the log.
//!
//! The consent: without ffmpeg a recording does not start but shows [`NEEDED_KEY`] with the
//! size, the source, the licence and the address of the source code; only the person's own
//! command `tools.ffmpeg.install` downloads ([`install`]), and no MCP tool reaches it. Nothing
//! touches the network before.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use rb_process::NoConsoleWindow as _;
use serde::Serialize;

use super::encoder::{BuildInfo, build_info};
use super::settings::VideoSettings;
use crate::cdp::lock;
use crate::tools::fetch::{Fetcher, HttpFetcher};
use crate::tools::manifest::megabytes;
use crate::tools::{Installed, ToolEntry, ToolError, ToolStore, ffmpeg_entry};

/// Video needs ffmpeg; placeholders `{version}`, `{size}`, `{license}`, `{source}`, `{from}`.
pub const NEEDED_KEY: &str = "video.notice.ffmpeg_needed";
/// The download started; the same placeholders.
pub const DOWNLOADING_KEY: &str = "video.notice.ffmpeg_downloading";
/// The download is done; placeholders `{version}`, `{folder}`.
pub const INSTALLED_KEY: &str = "video.notice.ffmpeg_installed";
/// The download failed; placeholder `{error}`.
pub const INSTALL_FAILED_KEY: &str = "video.notice.ffmpeg_failed";
/// A download is running already.
pub const INSTALL_BUSY_KEY: &str = "video.notice.ffmpeg_busy";
/// ffmpeg was removed; placeholder `{folder}`.
pub const REMOVED_KEY: &str = "video.notice.ffmpeg_removed";
/// There was no managed ffmpeg to remove.
pub const NOT_INSTALLED_KEY: &str = "video.notice.ffmpeg_not_installed";
/// The settings key the notices link to.
pub const SETTINGS_KEY: &str = "tools.ffmpegPath";
/// How long `ffmpeg -version` may take.
const PROBE_TIMEOUT: Duration = Duration::from_secs(15);

/// A download in progress.
static INSTALLING: Mutex<bool> = Mutex::new(false);
/// `ffmpeg -version` per program, read once.
static PROBED: Mutex<BTreeMap<PathBuf, BuildInfo>> = Mutex::new(BTreeMap::new());

/// Where the ffmpeg in use comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    /// `tools.ffmpegPath`.
    Custom,
    /// The managed download.
    Managed,
}

/// The ffmpeg a recording uses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ffmpeg {
    /// The program.
    pub path: PathBuf,
    /// Its origin.
    pub source: Source,
    /// The managed version; `None` for the user's own.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// The ffmpeg to use: the user's own, else the managed one; `None` when neither is there.
pub fn locate(settings: &VideoSettings, store: Option<&ToolStore>) -> Option<Ffmpeg> {
    if let Some(path) = &settings.ffmpeg_path {
        if path.is_file() {
            return Some(Ffmpeg {
                path: path.clone(),
                source: Source::Custom,
                version: None,
            });
        }
        tracing::warn!("tools.ffmpegPath names no file; the managed ffmpeg is used");
    }
    let entry = ffmpeg_entry().ok()?;
    managed(store?, &entry)
}

fn managed(store: &ToolStore, entry: &ToolEntry) -> Option<Ffmpeg> {
    store.installed(entry).map(|installed| Ffmpeg {
        path: installed.executable,
        source: Source::Managed,
        version: Some(installed.version),
    })
}

/// The managed store at its default place.
pub fn default_store() -> Option<ToolStore> {
    ToolStore::default_root().map(ToolStore::new)
}

/// Reads `ffmpeg -version` of `ffmpeg` (once per program) and refuses a managed build that is
/// no LGPL build.
///
/// # Errors
///
/// The program does not answer, or the managed build is GPL or non-free.
pub async fn probe(ffmpeg: &Ffmpeg) -> Result<BuildInfo, String> {
    let known = lock(&PROBED).get(&ffmpeg.path).cloned();
    let info = match known {
        Some(info) => info,
        None => {
            let info = build_info(&version_output(&ffmpeg.path).await?);
            lock(&PROBED).insert(ffmpeg.path.clone(), info.clone());
            info
        }
    };
    match (ffmpeg.source, info.is_lgpl()) {
        (Source::Managed, false) => Err(format!(
            "the downloaded ffmpeg at {} is no LGPL build; remove it and download it again",
            ffmpeg.path.display()
        )),
        (Source::Custom, false) => {
            tracing::info!("the ffmpeg of tools.ffmpegPath is no LGPL build (the user's own)");
            Ok(info)
        }
        _ => Ok(info),
    }
}

async fn version_output(path: &Path) -> Result<String, String> {
    let output = tokio::process::Command::new(path)
        .arg("-version")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .no_console_window()
        .output();
    let output = tokio::time::timeout(PROBE_TIMEOUT, output)
        .await
        .map_err(|_| format!("{} -version did not answer", path.display()))?
        .map_err(|error| format!("{} could not be started: {error}", path.display()))?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The consent notice: what would be downloaded, from where, under which licence, and where
/// its source code is.
pub fn needed_notice(entry: &ToolEntry) -> CoreToShell {
    notice(NotificationLevel::Warning, NEEDED_KEY, &entry_args(entry))
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
        ("source", entry.source_url.clone()),
        ("from", from),
    ]
}

/// A notification with the video settings linked.
pub fn notice(level: NotificationLevel, key: &str, args: &[(&str, String)]) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level,
        message_key: key.to_owned(),
        args: args
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect(),
        settings_key: Some(SETTINGS_KEY.to_owned()),
    })
}

/// Downloads and activates `entry` with `fetcher` into `store` (blocking), telling the person
/// through `notify`: what is downloaded, then the result. One download at a time.
///
/// # Errors
///
/// A download is running already, or the install failed (nothing new is active then).
pub fn install(
    store: &ToolStore,
    fetcher: &dyn Fetcher,
    entry: &ToolEntry,
    notify: &dyn Fn(CoreToShell),
) -> Result<Installed, ToolError> {
    {
        let mut installing = lock(&INSTALLING);
        if *installing {
            notify(notice(NotificationLevel::Info, INSTALL_BUSY_KEY, &[]));
            return Err(ToolError::Io("a download of ffmpeg is running".to_owned()));
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
    tracing::warn!(%error, "the ffmpeg download failed");
    notice(
        NotificationLevel::Error,
        INSTALL_FAILED_KEY,
        &[("error", error.to_string())],
    )
}

/// [`install`] over https into the default store.
///
/// # Errors
///
/// No tools folder, or as [`install`].
pub fn install_default(notify: &dyn Fn(CoreToShell)) -> Result<Installed, ToolError> {
    let prepared = default_store()
        .ok_or_else(|| ToolError::Io("no folder for tools".to_owned()))
        .and_then(|store| Ok((store, ffmpeg_entry()?)));
    match prepared {
        Ok((store, entry)) => install(&store, &HttpFetcher::new(), &entry, notify),
        Err(error) => {
            notify(failed(&error));
            Err(error)
        }
    }
}

/// Removes the managed ffmpeg; its folder, when there was one.
///
/// # Errors
///
/// No tools folder, or a file that cannot be deleted (a recording running).
pub fn remove_default() -> Result<Option<PathBuf>, ToolError> {
    let store = default_store().ok_or_else(|| ToolError::Io("no folder for tools".to_owned()))?;
    let folder = store.root().join(crate::tools::FFMPEG);
    lock(&PROBED).retain(|path, _| !path.starts_with(&folder));
    Ok(store.remove(crate::tools::FFMPEG)?.then_some(folder))
}

#[cfg(test)]
#[path = "ffmpeg_tests.rs"]
mod tests;
