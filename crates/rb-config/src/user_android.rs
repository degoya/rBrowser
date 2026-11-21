//! `engines.android`: Chrome on an Android device on USB as a view of the grid, over ADB
//! (RB-095-02). Every key is the person's: switching it on starts `adb`, the path names a
//! program to run, and `reverseLocalhost` (RB-096-12) lets the device reach one port of this
//! machine (`x-protected`, never over MCP).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `engines.android`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct AndroidSettings {
    /// An Android device on USB can be added as a view: Chrome on the device over ADB, its
    /// DevTools port forwarded to 127.0.0.1 only. The view has the device's own size. Off by
    /// default.
    pub enabled: bool,
    /// The adb executable; found automatically when empty (PATH, ANDROID_HOME,
    /// ANDROID_SDK_ROOT, the usual SDK folders, then the Platform Tools rBrowser downloaded
    /// after you agreed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adb_path: Option<String>,
    /// A view that loads localhost, 127.0.0.1 or ::1 reaches that port of this computer: rBrowser
    /// sets `adb reverse` for exactly that port while the view runs and removes it afterwards.
    /// While it stands, other apps on the device can reach that one port too. Off, the device
    /// reaches only itself at a loopback address. On by default.
    pub reverse_localhost: bool,
}

impl Default for AndroidSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            adb_path: None,
            reverse_localhost: true,
        }
    }
}

impl AndroidSettings {
    /// `adbPath` trimmed; blank is unset.
    pub fn adb_executable(&self) -> Option<&str> {
        self.adb_path
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())
    }
}

#[cfg(test)]
#[path = "user_android_tests.rs"]
mod tests;
