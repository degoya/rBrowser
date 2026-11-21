//! Which cloudflared runs the tunnel (RB-096-05), and getting the managed one: the person's own
//! (`tools.cloudflaredPath`, user settings only, an absolute path to a file) when set, else the
//! build under `%LOCALAPPDATA%\rBrowser\tools\cloudflared\<version>\` - never a path a project
//! names. Without either, the presentation does not start but says what would be downloaded
//! (size, licence, where from); only the person's own `portal.present.start` with `download`
//! downloads it ([`install`]), and no MCP tool reaches that. Nothing touches the network before.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;

use crate::cdp::lock;
use crate::tools::fetch::{Fetcher, HttpFetcher};
use crate::tools::manifest::megabytes;
use crate::tools::{CLOUDFLARED, Installed, ToolEntry, ToolError, ToolStore, cloudflared_entry};

/// A download in progress.
static INSTALLING: Mutex<bool> = Mutex::new(false);

/// What would be downloaded: for the consent in the Portal panel and its notice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Download {
    /// The pinned version.
    pub version: String,
    /// The size in megabytes, one decimal.
    pub size_mb: String,
    /// The SPDX licence.
    pub license: String,
    /// The host it comes from.
    pub from: String,
}

impl Download {
    /// What `entry` would download.
    #[must_use]
    pub fn of(entry: &ToolEntry) -> Self {
        Self {
            version: entry.version.clone(),
            size_mb: megabytes(entry.size),
            license: entry.license.clone(),
            from: entry
                .url
                .strip_prefix("https://")
                .and_then(|rest| rest.split('/').next())
                .unwrap_or_default()
                .to_owned(),
        }
    }

    /// The notice's placeholders.
    #[must_use]
    pub fn args(&self) -> Vec<(&'static str, String)> {
        vec![
            ("version", self.version.clone()),
            ("size", self.size_mb.clone()),
            ("license", self.license.clone()),
            ("from", self.from.clone()),
        ]
    }
}

/// The person's own cloudflared when `custom` names an absolute path to a file; else the
/// managed one in `store` for `entry`; `None` when neither is there.
#[must_use]
pub fn locate(
    custom: Option<&str>,
    store: Option<&ToolStore>,
    entry: Option<&ToolEntry>,
) -> Option<PathBuf> {
    if let Some(custom) = custom.map(str::trim).filter(|path| !path.is_empty()) {
        let path = Path::new(custom);
        if path.is_absolute() && path.is_file() {
            return Some(path.to_path_buf());
        }
        tracing::warn!("tools.cloudflaredPath names no absolute file; the managed one is used");
    }
    store?
        .installed(entry?)
        .map(|installed| installed.executable)
}

/// The cloudflared for the user settings' `tools.cloudflaredPath` and the default store.
#[must_use]
pub fn locate_default(custom: Option<&str>) -> Option<PathBuf> {
    let store = ToolStore::default_root().map(ToolStore::new);
    let entry = cloudflared_entry().ok();
    locate(custom, store.as_ref(), entry.as_ref())
}

/// What the default download would be; `None` where no build is pinned (another platform).
#[must_use]
pub fn download() -> Option<Download> {
    cloudflared_entry().ok().as_ref().map(Download::of)
}

/// Downloads and activates `entry` with `fetcher` into `store` (blocking); one at a time.
///
/// # Errors
///
/// A download is running already, or the install failed (nothing new is active then).
pub fn install(
    store: &ToolStore,
    fetcher: &dyn Fetcher,
    entry: &ToolEntry,
) -> Result<Installed, ToolError> {
    {
        let mut installing = lock(&INSTALLING);
        if *installing {
            return Err(ToolError::Busy {
                tool: CLOUDFLARED.to_owned(),
            });
        }
        *installing = true;
    }
    let result = store.install(fetcher, entry);
    *lock(&INSTALLING) = false;
    result
}

/// [`install`] over https into the default store.
///
/// # Errors
///
/// No tools folder, no pinned build, or as [`install`].
pub fn install_default() -> Result<Installed, ToolError> {
    let store = ToolStore::default_root()
        .map(ToolStore::new)
        .ok_or_else(|| ToolError::Io("no folder for tools".to_owned()))?;
    install(&store, &HttpFetcher::new(), &cloudflared_entry()?)
}

#[cfg(test)]
#[path = "cloudflared_tests.rs"]
mod tests;
