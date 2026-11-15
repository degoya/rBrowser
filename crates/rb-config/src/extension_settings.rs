//! `engines.chromium.extensions` of the user settings (RB-080-06, Engines > Chromium/CEF):
//! Chrome extensions from unpacked folders, loaded into every Chromium view of an instance at
//! its start, whether a trusted project's recommendations are offered, and the Chrome Web
//! Store (RB-080-23): whether installs from it are allowed and whether the start checks the
//! installed ones for updates. Only the person's own settings name a folder - never a project,
//! never MCP (`x-protected`). Read by `crates/rbrowser/src/extensions`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `engines.chromium.extensions`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct ChromiumExtensionSettings {
    /// Load the extensions below into the Chromium views. Off loads none; the list is kept.
    /// The change takes effect at the next start.
    pub enabled: bool,
    /// A trusted project's recommended extensions (`extensions` in its config) are offered in a
    /// notice; they are added only by the command Extensions: add the project's
    /// recommendations, never by themselves.
    pub offer_project_recommendations: bool,
    /// Unpacked extensions: each a folder with a Manifest V3 `manifest.json`, referenced where
    /// it is, not copied. Changes take effect at the next start.
    pub unpacked: Vec<UnpackedExtension>,
    /// Extensions from the Chrome Web Store (an id or a store URL) may be installed: downloaded
    /// from Google's update server, their CRX3 signatures checked, unpacked under rBrowser's
    /// local folder and loaded like an unpacked folder. Off hides the field and refuses the
    /// commands; what is installed stays loaded.
    pub allow_web_store: bool,
    /// At every start, ask the Web Store whether an installed extension has a newer version;
    /// an update is installed only after confirmation in the window. Off checks only on the
    /// command Extensions: check the Web Store for updates.
    pub check_updates_at_start: bool,
}

impl Default for ChromiumExtensionSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            offer_project_recommendations: true,
            unpacked: Vec::new(),
            allow_web_store: true,
            check_updates_at_start: false,
        }
    }
}

impl ChromiumExtensionSettings {
    /// The folders to load: every switched-on entry, none while the list is switched off.
    pub fn enabled_paths(&self) -> impl Iterator<Item = &str> {
        self.unpacked
            .iter()
            .filter(|entry| self.enabled && entry.enabled)
            .map(|entry| entry.path.as_str())
    }
}

/// One entry of `engines.chromium.extensions.unpacked`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UnpackedExtension {
    /// The absolute path of the extension's folder (the one holding `manifest.json`); a path
    /// to the `manifest.json` itself names its folder.
    pub path: String,
    /// Load this extension at the next start.
    #[serde(default = "loaded")]
    pub enabled: bool,
}

const fn loaded() -> bool {
    true
}

#[cfg(test)]
#[path = "extension_settings_tests.rs"]
mod tests;
