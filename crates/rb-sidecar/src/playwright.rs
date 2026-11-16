//! The Playwright browser download (`engines.update`): the WebKit build that matches the pinned
//! playwright-core, into `%LOCALAPPDATA%\rBrowser\ms-playwright` (or
//! `engines.playwrightBrowsersPath`), with progress. The sidecar's `playwright.install` does the
//! work and recognises an existing installation.

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::error::SidecarError;
use crate::settings::non_empty;
use crate::supervisor::{Launcher, Sidecar};

/// The sidecar method.
pub const INSTALL: &str = "playwright.install";
/// How long a download may take before it is cancelled.
pub const INSTALL_TIMEOUT: Duration = Duration::from_secs(30 * 60);
/// The i18n key of a progress line: `{browser}`, `{percent}`.
pub const PROGRESS_KEY: &str = "sidecar.install.progress";
/// The i18n key after a download: `{browser}`, `{version}`.
pub const DONE_KEY: &str = "sidecar.install.done";
/// The i18n key when the build was there already: `{browser}`, `{version}`.
pub const ALREADY_KEY: &str = "sidecar.install.alreadyInstalled";

/// The browsers folder: `setting` (`engines.playwrightBrowsersPath`, absolute) when set, else
/// `<local_app_data>\rBrowser\ms-playwright`.
pub fn browsers_path(
    setting: Option<&str>,
    local_app_data: Option<OsString>,
) -> Result<PathBuf, SidecarError> {
    if let Some(setting) = non_empty(setting) {
        let path = PathBuf::from(setting);
        return if path.is_absolute() {
            Ok(path)
        } else {
            Err(SidecarError::BrowsersPath(format!(
                "{} is not an absolute path",
                path.display()
            )))
        };
    }
    local_app_data
        .filter(|dir| !dir.is_empty())
        .map(|dir| PathBuf::from(dir).join("rBrowser").join("ms-playwright"))
        .ok_or_else(|| SidecarError::BrowsersPath("LOCALAPPDATA is not set".to_owned()))
}

/// A progress update of a download.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct InstallProgress {
    /// The browser (`webkit`).
    pub browser: String,
    /// `download` or `done`.
    pub stage: String,
    /// 0 to 100.
    pub percent: u8,
}

/// What `playwright.install` did.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallReport {
    /// The browser.
    pub browser: String,
    /// Playwright's build number.
    pub revision: String,
    /// The browser's version, when Playwright names one.
    pub version: Option<String>,
    /// The build's folder.
    pub path: PathBuf,
    /// The build was there already; nothing was downloaded.
    pub already_installed: bool,
}

impl InstallReport {
    /// The i18n key of the closing message.
    pub fn i18n_key(&self) -> &'static str {
        if self.already_installed {
            ALREADY_KEY
        } else {
            DONE_KEY
        }
    }
}

/// Installs `browser` (only `webkit`), calling `on_progress` for each update. Starts the sidecar
/// if needed; dropping the future cancels the download.
pub async fn install<L: Launcher>(
    sidecar: &Sidecar<L>,
    browser: &str,
    mut on_progress: impl FnMut(InstallProgress),
) -> Result<InstallReport, SidecarError> {
    let (progress, mut updates) = mpsc::unbounded_channel();
    let request = sidecar.request_with_progress(
        INSTALL,
        json!({ "browser": browser }),
        Some(INSTALL_TIMEOUT),
        progress,
    );
    tokio::pin!(request);
    let answer = loop {
        tokio::select! {
            answer = &mut request => break answer,
            Some(update) = updates.recv() => forward(update, &mut on_progress),
        }
    };
    while let Ok(update) = updates.try_recv() {
        forward(update, &mut on_progress);
    }
    serde_json::from_value(answer?).map_err(|error| SidecarError::Protocol(error.to_string()))
}

fn forward(update: Value, on_progress: &mut impl FnMut(InstallProgress)) {
    if let Ok(progress) = serde_json::from_value(update) {
        on_progress(progress);
    }
}

#[cfg(test)]
#[path = "playwright_tests.rs"]
mod tests;
