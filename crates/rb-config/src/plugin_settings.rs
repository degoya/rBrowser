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
    /// Let the consent probe of a plugin granted `consent-probe` load any address. Off holds it
    /// to public addresses and the trusted project's own sites, so a plugin cannot run its
    /// script in a local or LAN page (RB-094-09).
    pub consent_probe_any_url: bool,
    /// The largest message between a plugin panel and rBrowser, as JSON text in bytes, in
    /// either direction (RB-090-44): a call's arguments, its answer, an event. A page script's
    /// messages keep their own 64 KiB.
    #[schemars(range(min = 65_536, max = 16_777_216))]
    pub panel_message_max_bytes: u32,
    /// The plugin catalogue (RB-091-01): Settings > Plugins > Catalog.
    pub catalog: CatalogSettings,
}

/// `plugins.catalog`: the signed plugin catalogue (RB-091-01) and the bundled first-party
/// plugins (RB-091-04).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct CatalogSettings {
    /// Look for updates of the plugins installed from the catalogue at every start, and say so
    /// in the status bar when there are some.
    pub check_on_start: bool,
    /// Plugin repositories besides the official catalogue: each an https address of its
    /// `rbrowser-plugin-index.json` and the public key its index is signed with.
    #[schemars(length(max = 16))]
    pub repositories: Vec<CatalogRepository>,
    /// Install an update of a plugin by itself when the update check finds one that is signed
    /// by the same key and asks for nothing more (RB-091-04); one that asks for more waits for a
    /// click in the catalogue.
    pub auto_update: bool,
    /// Install a chosen first-party plugin from the official repository when it offers the
    /// bundled version or a newer one (RB-091-04); off, every chosen plugin comes from the
    /// bundle beside rBrowser and nothing reaches the network.
    pub prefer_repository: bool,
    /// Say in the status bar when the project's config names a plugin that is not installed
    /// (RB-091-04).
    pub missing_notice: bool,
    /// Name the plugins new in rBrowser's bundle once after the update that brought them, with
    /// a button that installs them (RB-094-13).
    pub new_notice: bool,
}

impl Default for CatalogSettings {
    fn default() -> Self {
        Self {
            check_on_start: false,
            repositories: Vec::new(),
            auto_update: false,
            prefer_repository: true,
            missing_notice: true,
            new_notice: true,
        }
    }
}

/// A plugin repository a person added to the catalogue.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogRepository {
    /// What the catalogue calls it.
    #[serde(default)]
    #[schemars(length(max = 80))]
    pub name: String,
    /// The https address of the repository's `rbrowser-plugin-index.json`.
    #[schemars(regex(pattern = r"^https://"), length(max = 1024))]
    pub url: String,
    /// The repository's Ed25519 public key, base64 (as `rb-plugin-pack keygen` prints it).
    #[schemars(regex(pattern = r"^[A-Za-z0-9+/]{43}=$"))]
    pub public_key: String,
}

impl PluginSettings {
    /// `panelMessageMaxBytes` by default: 4 MiB, the most JSON a plugin's component answers.
    pub const PANEL_MESSAGE_DEFAULT_BYTES: u32 = 4 * 1024 * 1024;
    /// The smallest `panelMessageMaxBytes`: the page scripts' 64 KiB.
    pub const PANEL_MESSAGE_MIN_BYTES: u32 = 64 * 1024;
    /// The largest `panelMessageMaxBytes`: 16 MiB.
    pub const PANEL_MESSAGE_MAX_BYTES: u32 = 16 * 1024 * 1024;

    /// The panel message limit in effect, in bytes: `panelMessageMaxBytes` held to its range (a
    /// hand-edited file may hold anything).
    #[must_use]
    pub fn panel_message_limit(&self) -> usize {
        let bytes = self
            .panel_message_max_bytes
            .clamp(Self::PANEL_MESSAGE_MIN_BYTES, Self::PANEL_MESSAGE_MAX_BYTES);
        usize::try_from(bytes).unwrap_or(usize::MAX)
    }
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
            consent_probe_any_url: false,
            panel_message_max_bytes: Self::PANEL_MESSAGE_DEFAULT_BYTES,
            catalog: CatalogSettings::default(),
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
