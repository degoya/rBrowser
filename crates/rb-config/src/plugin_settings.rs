//! The `plugins` section of the user settings (RB-010-03, `docs/plugins.md`), split from
//! `user_settings.rs` to keep both files under 500 lines.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `plugins`: Settings > Plugins > Installed (RB-010-03, `docs/plugins.md`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct PluginSettings {
    /// The ids of the plugins switched off. The settings window offers the installed plugins
    /// by name (`x-options`, RB-080-72).
    #[schemars(extend("uniqueItems" = true, "x-options" = "plugins"))]
    pub disabled: Vec<PluginId>,
    /// The signing keys trusted on first use (pinned by fingerprint).
    pub trusted_keys: Vec<TrustedPluginKey>,
    /// Load unsigned plugins from the plugin folders (development only).
    pub development_mode: bool,
    /// Failures after which a plugin is suspended until it is resumed.
    #[schemars(range(min = 1, max = 10))]
    pub crash_limit: u32,
    /// Keep the compiled plugins on disk between starts.
    pub compile_cache: bool,
    /// The ids of the plugins whose page scripts do not run in the views (RB-060-12); the
    /// window offers the plugins that have page scripts.
    #[schemars(extend("uniqueItems" = true, "x-options" = "plugin-page-scripts"))]
    pub page_scripts_disabled: Vec<PluginId>,
    /// The ids of the plugins whose panels the shell does not offer (RB-060-12); the window
    /// offers the plugins that have panels.
    #[schemars(extend("uniqueItems" = true, "x-options" = "plugin-panels"))]
    pub panels_hidden: Vec<PluginId>,
    /// List the plugins that ship with rBrowser in Settings > Plugins (RB-080-51).
    pub show_bundled: bool,
    /// Let plugins run Lighthouse on any address. Off holds a run a plugin starts to the sites
    /// the views show, so it cannot read local or LAN pages through Lighthouse's report
    /// (RB-090-21).
    pub lighthouse_any_url: bool,
}

impl Default for PluginSettings {
    fn default() -> Self {
        Self {
            disabled: Vec::new(),
            trusted_keys: Vec::new(),
            development_mode: false,
            crash_limit: 3,
            compile_cache: true,
            page_scripts_disabled: Vec::new(),
            panels_hidden: Vec::new(),
            show_bundled: true,
            lighthouse_any_url: false,
        }
    }
}

/// A plugin id: lower-case words joined by `-` (`overlay-grid`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct PluginId(#[schemars(regex(pattern = r"^[a-z][a-z0-9]*(-[a-z0-9]+)*$"))] pub String);

/// A trusted plugin signing key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrustedPluginKey {
    /// The key's id as the package names it.
    pub key_id: String,
    /// The public key (PEM or base64, as `rb-plugin-pack` writes it).
    pub public_key: String,
    /// SHA-256 of the public key, lower-case hex.
    #[schemars(regex(pattern = r"^[0-9a-f]{64}$"))]
    pub fingerprint: String,
}
