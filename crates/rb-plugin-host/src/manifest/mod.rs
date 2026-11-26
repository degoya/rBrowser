//! `manifest.toml`: what a plugin is, what it may do and what it contributes
//! (`docs/plugins.md`, "The manifest").
//!
//! Adapted from rCommander's `crates/rc-plugin-host/src/manifest.rs` (itself from rDownloader's):
//! the same shape - a strict TOML document (`deny_unknown_fields`), bounded in size, every field
//! checked before anything else of the package is looked at - with rBrowser's fields. Decided
//! here (RB-010-03, the open points of `docs/plugins.md`):
//!
//! - **Identity**: a readable id (`breakpoint-analyzer`): lower-case words joined by single
//!   hyphens, 2 to 48 characters, no dots - so no reverse-DNS form - and no UUID. Unique per
//!   installation; the host refuses a second plugin with the same id.
//! - **MCP prefix**: one `mcp_prefix` per plugin, apart from the id (`breakpoints` for the
//!   `breakpoint-analyzer`), lower-case letters and digits only, so `<prefix>_<tool>` splits in
//!   one way. Required when the plugin declares tools. Planned tool names that do not share one
//!   prefix are renamed (`overflow_check` becomes `overlay_overflow_check`).
//! - **Locales**: `[locales] en = "locales/en.json"`, `en` required, each a package member in
//!   the catalogue's nested shape with every key below `plugin.<id>.` (`docs/i18n.md`).

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail};
use rb_plugin_types::{Capability, PluginKind};
use serde::Deserialize;

use crate::PluginLimits;

mod checks;
mod grants;

pub use checks::{RESERVED_IDS, RESERVED_MCP_PREFIXES, valid_mcp_prefix, valid_plugin_id};
pub use grants::{
    DEFAULT_HTTP_RATE_PER_SECOND, Grants, MAX_HTTP_RATE_PER_SECOND, PLUGIN_SIDECAR_NAMESPACES,
};

/// The manifest format this host reads.
pub const MANIFEST_VERSION: u32 = 1;
/// The largest `manifest.toml` read at all.
pub const MAX_MANIFEST_BYTES: usize = 64 * 1024;
/// The contract versions this host runs.
pub const SUPPORTED_API_VERSIONS: &[&str] = &[rb_plugin_api::API_VERSION];
/// The package member a manifest's `[settings]` schema is read from.
pub const SETTINGS_SCHEMA_MEMBER: &str = "settings.schema.json";

/// A parsed and checked manifest.
#[derive(Clone, Debug, PartialEq)]
pub struct PluginManifest {
    pub id: String,
    /// The English display name; a translation is `plugin.<id>.name` in its locales.
    pub name: String,
    pub version: semver::Version,
    pub api_version: String,
    pub kinds: Vec<PluginKind>,
    pub mcp_prefix: Option<String>,
    pub key_id: String,
    /// Base64 Ed25519 public key the package is signed with.
    pub public_key: String,
    pub description: String,
    pub metadata: Metadata,
    pub grants: Grants,
    pub limits: PluginLimits,
    /// Whether the package carries [`SETTINGS_SCHEMA_MEMBER`].
    pub settings: bool,
    /// `[locales]`: language to package member (`en` to `locales/en.json`).
    pub locales: BTreeMap<String, String>,
    pub commands: Vec<CommandEntry>,
    pub tools: Vec<ToolEntry>,
    pub page_scripts: Vec<PageScript>,
    pub panels: Vec<Panel>,
    pub cli: Vec<CliEntry>,
    /// `context_menu`: the ids of `[[commands]]` the page's context menu offers under
    /// Checks (RB-080-43), in this order; each once, each a declared command.
    pub context_menu: Vec<String>,
}

/// `[metadata]`.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub homepage: Option<String>,
    /// The first rBrowser release this plugin runs on.
    #[serde(default)]
    pub min_app_version: Option<String>,
}

/// `[[commands]]`: a command in the one registry, `plugin.<id>.<command>`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CommandEntry {
    /// The last segment of the command id: `[a-z][a-z0-9_]*`.
    pub id: String,
    /// An i18n key of the plugin's locales: `plugin.<id>.*`.
    pub title: String,
    /// The default binding in the text form of `docs/shortcuts.md` (`Ctrl+Shift+Space X A`).
    #[serde(default)]
    pub shortcut: Option<String>,
    /// The leader group's id; default `plugins` (`X`).
    #[serde(default)]
    pub group: Option<String>,
    /// The tool of `[[tools]]` covering it over MCP.
    #[serde(default)]
    pub tool: Option<String>,
    /// Why no tool covers it (`ui-only: ...`); exactly one of `tool` and this.
    #[serde(default)]
    pub mcp_omitted: Option<String>,
    /// The views the command runs over when its call names none (RB-099-10): `"visible"` or
    /// breakpoint ids. The host hands the plugin `$scope` and the breakpoints it resolves to as
    /// `breakpoints`.
    #[serde(default)]
    pub scope: Option<rb_config::ViewScope>,
}

/// `[[tools]]`: an MCP tool, `<mcp_prefix>_<name>`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ToolEntry {
    /// `[a-z][a-z0-9_]*`, at most 40 characters.
    pub name: String,
    /// An i18n key of the plugin's locales.
    pub description: String,
    /// The JSON schema of the arguments, written as TOML; default `{ type = "object" }`.
    #[serde(default)]
    pub input: Option<toml::Table>,
    /// The tool runs JavaScript its caller supplies: over MCP it answers only while the person
    /// allows `evaluate` (`mcp.allowEvaluate`), like the core's `evaluate` (Charge 6).
    #[serde(default)]
    pub evaluates_input: bool,
}

/// `[[page_scripts]]`: a script injected into every view's isolated world.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PageScript {
    /// A package member under `page/`.
    pub path: String,
    /// `document-start`, `document-end` or `document-idle` (the default).
    #[serde(default = "default_run_at")]
    pub run_at: String,
}

fn default_run_at() -> String {
    "document-idle".to_owned()
}

/// `[[panels]]`: a panel or dialog of the shell.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Panel {
    pub id: String,
    /// An i18n key of the plugin's locales.
    pub title: String,
    /// A package member under `panel/`.
    pub entry: String,
    /// Where the shell shows it (RB-060-12); default [`PanelPlacement::Sidebar`].
    #[serde(default)]
    pub placement: PanelPlacement,
    /// Its icon in the panels' side bar (RB-080-61): a Lucide icon name, `[a-z0-9-]{1,40}`, never
    /// a URL or markup. The shell shows only the icons it bundles and a fallback for any other.
    #[serde(default)]
    pub icon: Option<String>,
}

/// Where the shell shows a panel: a tab of the panels window's side bar, or a dialog over it.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelPlacement {
    #[default]
    Sidebar,
    Dialog,
}

/// `[[cli]]`: `rbrowser <plugin-id> <name>` runs a command.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CliEntry {
    pub name: String,
    /// The `id` of one of `[[commands]]`.
    pub command: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    manifest_version: u32,
    id: String,
    name: String,
    version: String,
    api_version: String,
    kinds: Vec<String>,
    #[serde(default)]
    mcp_prefix: Option<String>,
    key_id: String,
    public_key: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    metadata: Metadata,
    #[serde(default)]
    capabilities: toml::Table,
    #[serde(default)]
    limits: PluginLimits,
    #[serde(default)]
    settings: Option<RawSettings>,
    locales: BTreeMap<String, String>,
    #[serde(default)]
    commands: Vec<CommandEntry>,
    #[serde(default)]
    tools: Vec<ToolEntry>,
    #[serde(default)]
    page_scripts: Vec<PageScript>,
    #[serde(default)]
    panels: Vec<Panel>,
    #[serde(default)]
    cli: Vec<CliEntry>,
    #[serde(default)]
    context_menu: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSettings {
    schema: String,
}

/// Parses and checks `manifest.toml`.
///
/// # Errors
///
/// The text is too large, no UTF-8 or no valid TOML, a field is unknown, or a check of
/// `checks.rs` refuses it; the message names the field.
pub fn parse_manifest(bytes: &[u8]) -> Result<PluginManifest> {
    if bytes.len() > MAX_MANIFEST_BYTES {
        bail!("manifest.toml exceeds {MAX_MANIFEST_BYTES} bytes");
    }
    let text = std::str::from_utf8(bytes).context("manifest.toml is not UTF-8")?;
    let raw: RawManifest = toml::from_str(text).context("parse manifest.toml")?;
    if raw.manifest_version != MANIFEST_VERSION {
        bail!(
            "manifest_version {} is not supported (expected {MANIFEST_VERSION})",
            raw.manifest_version
        );
    }
    let mut kinds = Vec::new();
    for kind in &raw.kinds {
        let kind: PluginKind = kind.parse().with_context(|| format!("kinds: {kind}"))?;
        if kinds.contains(&kind) {
            bail!("kinds names {kind} twice");
        }
        kinds.push(kind);
    }
    if let Some(settings) = &raw.settings
        && settings.schema != SETTINGS_SCHEMA_MEMBER
    {
        bail!("settings.schema must be \"{SETTINGS_SCHEMA_MEMBER}\"");
    }
    let manifest = PluginManifest {
        version: semver::Version::parse(&raw.version)
            .with_context(|| format!("version {} is no SemVer version", raw.version))?,
        grants: Grants::parse(&raw.capabilities)?,
        id: raw.id,
        name: raw.name,
        api_version: raw.api_version,
        kinds,
        mcp_prefix: raw.mcp_prefix,
        key_id: raw.key_id,
        public_key: raw.public_key,
        description: raw.description,
        metadata: raw.metadata,
        limits: raw.limits,
        settings: raw.settings.is_some(),
        locales: raw.locales,
        commands: raw.commands,
        tools: raw.tools,
        page_scripts: raw.page_scripts,
        panels: raw.panels,
        cli: raw.cli,
        context_menu: raw.context_menu,
    };
    checks::check(&manifest)?;
    Ok(manifest)
}

impl PluginManifest {
    /// Whether the manifest grants `capability`.
    #[must_use]
    pub fn grants(&self, capability: Capability) -> bool {
        self.grants.granted.contains(&capability)
    }

    /// Whether the plugin is of `kind`.
    #[must_use]
    pub fn is(&self, kind: PluginKind) -> bool {
        self.kinds.contains(&kind)
    }

    /// The command id `plugin.<id>.<command>`, a `-` of the id written `_` (contract 0.1).
    #[must_use]
    pub fn command_id(&self, command: &str) -> String {
        format!(
            "{}{command}",
            rb_commands::registry::plugin_command_prefix(&self.id)
        )
    }

    /// The MCP tool name `<mcp_prefix>_<tool>`; `None` without a prefix.
    #[must_use]
    pub fn tool_name(&self, tool: &str) -> Option<String> {
        self.mcp_prefix
            .as_ref()
            .map(|prefix| format!("{prefix}_{tool}"))
    }

    /// The prefix every i18n key of the plugin starts with: `plugin.<id>.`.
    #[must_use]
    pub fn key_prefix(&self) -> String {
        format!("plugin.{}.", self.id)
    }

    /// The package members the manifest names: locales, page scripts and panel entries.
    #[must_use]
    pub fn named_members(&self) -> BTreeSet<&str> {
        self.locales
            .values()
            .map(String::as_str)
            .chain(self.page_scripts.iter().map(|script| script.path.as_str()))
            .chain(self.panels.iter().map(|panel| panel.entry.as_str()))
            .collect()
    }
}

#[cfg(test)]
#[path = "manifest_tests.rs"]
mod tests;
