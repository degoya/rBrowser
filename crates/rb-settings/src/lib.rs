//! The settings structure of rBrowser: categories and sections, scopes, the schema annotations
//! every key carries, the setting definitions and the registry (contract 0.1,
//! `docs/contracts-0.1.md`).
//!
//! The types are the contract; the registry's behaviour ([`SettingsRegistry`]), the reading of
//! the schema annotations ([`defs_from_schema`]), the scope resolution ([`LayeredStore`]) and the
//! deep links ([`SettingsRegistry::resolve_deep_link`]) are RB-010-02's; the lint that holds the
//! schemas to the same rules without compiling is `scripts/check-settings.sh`. The settings
//! window (RB-010-08) and MCP `settings_get`/`settings_set` (RB-030-03) read through them. This
//! crate depends on no other rBrowser crate: `rb-config` uses its [`Scope`] and [`Origin`] and
//! annotates its schema with the names below.

mod annotate;
mod category;
mod deeplink;
mod registry;
mod scope;
mod secret;
#[cfg(test)]
mod test_support;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use annotate::{STRUCTURAL_KEYS, SchemaIssue, defs_from_schema, plugin_defs_from_schema};
pub use category::{Category, SECTIONS, Section, section};
pub use deeplink::{DeepLinkTarget, deep_link};
pub use registry::SettingsRegistry;
pub use scope::LayeredStore;
pub use secret::{EditorPanel, SECRET_MASK, mask_secret};

/// Schema annotation: the [`Category`] id of a key. Required on every key (the lint fails
/// without it).
pub const X_CATEGORY: &str = "x-category";
/// Schema annotation: the section id inside the category. Required on every key.
pub const X_SECTION: &str = "x-section";
/// Schema annotation: the position inside the section (an integer, unique per section).
/// Required on every key.
pub const X_ORDER: &str = "x-order";
/// Schema annotation: the scopes a key may be written to, an array of [`Scope`] ids.
pub const X_SCOPE: &str = "x-scope";
/// Schema annotation: `true` when the key is valid only in the Local scope
/// (`.rbrowser/config.local.json`); in `config.json` it is ignored with a warning.
pub const X_LOCAL_ONLY: &str = "x-local-only";
/// Schema annotation: `true` when MCP may neither read-modify nor write the key (trust, plugins,
/// MCP itself).
pub const X_PROTECTED: &str = "x-protected";
/// Schema annotation: the i18n key of the label; default `settings.key.<key>.label`.
pub const X_LABEL_KEY: &str = "x-label-key";
/// Schema annotation: the i18n key of the description; default
/// `settings.key.<key>.description`.
pub const X_DESCRIPTION_KEY: &str = "x-description-key";
/// Schema annotation: the i18n key of the help text (what the key is for, its effect, an
/// example; the window adds the default); default `settings.key.<key>.help` (RB-080-20).
pub const X_HELP_KEY: &str = "x-help-key";
/// Schema annotation on a string: which native dialog fills it, `"file"` or `"folder"`
/// (RB-080-20); the window shows Browse beside the field.
pub const X_INPUT: &str = "x-input";
/// Schema annotation: `true` when the key's values are secrets (RB-094-26, RB-094-27): the key is
/// not in the settings window at all, MCP's `settings_get` masks its values. Only on an
/// `x-protected` key (the settings lint).
pub const X_SECRET: &str = "x-secret";
/// Schema annotation on an `x-secret` key: `{ "panel": "<id>", "tab": "<id>" }`, the panel of the
/// key's plugin (and its tab) where its values are edited; a link to the key opens it (RB-094-27).
pub const X_PANEL: &str = "x-panel";
/// Schema annotation on a string: suggested values the window offers while the field stays
/// free text (`"Win32"`, `"Europe/Berlin"`; RB-080-20).
pub const X_OPTIONS: &str = "x-options";

/// The prefix of a settings deep link: `rbrowser://settings/sync.scroll` opens the key
/// `sync.scroll`. The parser is RB-010-02's.
pub const DEEP_LINK_PREFIX: &str = "rbrowser://settings/";

/// Where a value may be stored. Serialized lowercase (`"user"`, `"project"`, `"local"`).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// `%APPDATA%\rBrowser\settings.json`: app values and shortcuts.
    User,
    /// `.rbrowser/config.json`, checked in: what the team shares.
    Project,
    /// `.rbrowser/config.local.json`, git-ignored: overrides for this machine, local-only keys.
    Local,
}

/// Where the effective value of a key comes from: a [`Scope`] or the schema default. Shown as
/// the badge on every field. Serialized lowercase.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    /// No file sets it; the schema default applies.
    Default,
    /// The user settings.
    User,
    /// `config.json`.
    Project,
    /// `config.local.json`.
    Local,
}

impl From<Scope> for Origin {
    fn from(scope: Scope) -> Self {
        match scope {
            Scope::User => Origin::User,
            Scope::Project => Origin::Project,
            Scope::Local => Origin::Local,
        }
    }
}

/// One setting key with its place in the window and its rules - the schema annotations of the
/// key, read into a struct by [`defs_from_schema`] (config and user-settings schemas) and
/// [`plugin_defs_from_schema`] (the plugins' schema fragments).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingDef {
    /// The dotted key, e.g. `sync.scroll` or `network.ignoreCertificateErrors`.
    pub key: String,
    /// `x-category`.
    pub category: Category,
    /// `x-section`: a section id of [`SettingDef::category`].
    pub section: String,
    /// `x-order`.
    pub order: u32,
    /// `x-scope`: the scopes the key may be written to.
    pub scope_allowed: Vec<Scope>,
    /// `x-local-only`.
    pub local_only: bool,
    /// `x-protected`: refused for every MCP caller.
    pub protected_from_mcp: bool,
    /// `x-label-key`.
    pub label_key: String,
    /// `x-description-key`.
    pub description_key: String,
    /// `x-help-key`.
    pub help_key: String,
    /// The key's own JSON schema (type, default, enum, ...), annotations included.
    pub schema: Value,
}

/// The effective value of a key and where it comes from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedValue {
    /// The value after merging default, user, project and local.
    pub value: Value,
    /// The layer that supplied it.
    pub origin: Origin,
}

/// What can go wrong when registering, resolving or writing a setting.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum SettingsError {
    /// The contract stub: the job that implements the function has not landed yet.
    #[error("not implemented yet (contract 0.1 stub)")]
    NotImplemented,
    /// No setting has this key.
    #[error("unknown setting key `{0}`")]
    UnknownKey(String),
    /// A definition names a category that does not exist (plugins cannot add one).
    #[error("unknown category `{0}`")]
    UnknownCategory(String),
    /// A definition names a section its category does not have.
    #[error("unknown section `{section}` in category `{category}`")]
    UnknownSection {
        /// The category id.
        category: String,
        /// The section id.
        section: String,
    },
    /// A key is registered twice.
    #[error("setting key `{0}` registered twice")]
    Duplicate(String),
    /// The key may not be written to this scope (a local-only key to Project, for example).
    #[error("setting `{key}` cannot be written to scope `{scope:?}`")]
    ScopeNotAllowed {
        /// The key.
        key: String,
        /// The refused scope.
        scope: Scope,
    },
    /// The key is protected from MCP.
    #[error("setting `{0}` is protected")]
    Protected(String),
    /// The value does not match the key's schema.
    #[error("invalid value for `{key}`: {reason}")]
    Invalid {
        /// The key.
        key: String,
        /// Why, in English (the UI shows the i18n text of the validation).
        reason: String,
    },
}

/// Reads and writes setting values across the scopes. [`LayeredStore`] implements it over the
/// three documents `rb-config` loads; the shell IPC (RB-010-07/RB-010-08) and MCP (RB-030-03)
/// call it.
pub trait SettingsStore: Send + Sync {
    /// The effective value of `key` and its origin.
    fn resolve(&self, key: &str) -> Result<ResolvedValue, SettingsError>;

    /// Writes `value` to `scope`; `Value::Null` removes the key from that scope (reset). Refuses
    /// a scope the key does not allow.
    fn set(&self, key: &str, value: Value, scope: Scope) -> Result<(), SettingsError>;
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
