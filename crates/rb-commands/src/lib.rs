//! The command registry: every function of rBrowser is a command with an id, a title, a leader
//! group, default bindings and an MCP mapping; toolbar, menus, palette, shortcuts and MCP all
//! dispatch the same command (contract 0.1, `docs/contracts-0.1.md`, `docs/shortcuts.md`).
//!
//! The types, the text form of bindings ([`keys`]) and the reservation list's format
//! ([`reserved`]) are the contract. This crate also holds the registry's behaviour
//! ([`registry`], [`user_bindings`]), the binding rules ([`validate`]), the leader state machine
//! with the which-key data ([`chord`]), the key mapping from CEF events ([`keymap`]), the lint
//! with the coverage check ([`lint`]) and its own commands ([`builtin`]) (RB-010-17). The other
//! commands are registered by the jobs that own them (RB-010-04, -05, -07, -12, -16, ...).

pub mod builtin;
pub mod builtin_features;
pub mod chord;
pub mod keymap;
pub mod keys;
pub mod lint;
pub mod registry;
pub mod reserved;
pub mod retired;
pub mod user_bindings;
pub mod validate;

use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use chord::{ChordMachine, ChordSettings, Focus, KeyOutcome, WhichKey, WhichKeyEntry};
pub use keys::{Binding, Chord, Key, LEADER, Modifiers, Shortcut};
pub use registry::{CommandRegistry, CommandSummary, Notice};
pub use reserved::{
    ContextException, KeyContext, Platform, RESERVED_SHORTCUTS_PATH, ReservedEntry, ReservedList,
    ReservedSource, builtin_reserved, reserved_list,
};

/// A command id: dot-separated segments of `a-z`, `0-9` and `_`, at least two, the first
/// starting with a letter. The first segment is the area (`view`, `sync`, `layout`, `env`,
/// `emulation`, `app`, `plugin`), parameters may be a trailing segment
/// (`view.set_breakpoint.2`). Plugin commands are `plugin.<plugin_id>.<command>`, a `-` in the
/// plugin id written as `_`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CommandId(String);

impl CommandId {
    /// Checks `id` against the format.
    pub fn new(id: &str) -> Result<Self, CommandError> {
        let mut segments = id.split('.');
        let first_ok = segments
            .next()
            .and_then(|first| first.chars().next())
            .is_some_and(|c| c.is_ascii_lowercase());
        let all_ok = id.split('.').count() >= 2
            && id.split('.').all(|segment| {
                !segment.is_empty()
                    && segment
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            });
        if first_ok && all_ok {
            Ok(Self(id.to_owned()))
        } else {
            Err(CommandError::InvalidId(id.to_owned()))
        }
    }

    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandId {
    type Error = CommandError;

    fn try_from(id: String) -> Result<Self, Self::Error> {
        CommandId::new(&id)
    }
}

impl From<CommandId> for String {
    fn from(id: CommandId) -> Self {
        id.0
    }
}

impl fmt::Display for CommandId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A leader group: the first key after the leader (`docs/shortcuts.md`, "The leader scheme").
/// Also the grouping of Settings > Keyboard Shortcuts. Serialized as [`Group::id`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Group {
    /// `B`.
    Breakpoints,
    /// `L`.
    Layout,
    /// `S`.
    Sync,
    /// `E`.
    Environments,
    /// `D` - DevTools & Debugging.
    Devtools,
    /// `O`.
    Overlays,
    /// `C`.
    Capture,
    /// `K`.
    Checks,
    /// `R`.
    Reload,
    /// `P`.
    Panels,
    /// `F` - Flows & Recorder.
    Flows,
    /// `V`.
    View,
    /// `X`.
    Plugins,
    /// `,`.
    Settings,
}

impl Group {
    /// Every group in the order of the docs' table.
    pub const ALL: [Group; 14] = [
        Group::Breakpoints,
        Group::Layout,
        Group::Sync,
        Group::Environments,
        Group::Devtools,
        Group::Overlays,
        Group::Capture,
        Group::Checks,
        Group::Reload,
        Group::Panels,
        Group::Flows,
        Group::View,
        Group::Plugins,
        Group::Settings,
    ];

    /// The key after the leader that opens the group.
    pub const fn leader_key(self) -> char {
        match self {
            Group::Breakpoints => 'B',
            Group::Layout => 'L',
            Group::Sync => 'S',
            Group::Environments => 'E',
            Group::Devtools => 'D',
            Group::Overlays => 'O',
            Group::Capture => 'C',
            Group::Checks => 'K',
            Group::Reload => 'R',
            Group::Panels => 'P',
            Group::Flows => 'F',
            Group::View => 'V',
            Group::Plugins => 'X',
            Group::Settings => ',',
        }
    }

    /// The id: the lowercase variant name (`breakpoints`, ..., `settings`).
    pub fn id(self) -> &'static str {
        match self {
            Group::Breakpoints => "breakpoints",
            Group::Layout => "layout",
            Group::Sync => "sync",
            Group::Environments => "environments",
            Group::Devtools => "devtools",
            Group::Overlays => "overlays",
            Group::Capture => "capture",
            Group::Checks => "checks",
            Group::Reload => "reload",
            Group::Panels => "panels",
            Group::Flows => "flows",
            Group::View => "view",
            Group::Plugins => "plugins",
            Group::Settings => "settings",
        }
    }

    /// The i18n key of the group's title: `commands.group.<id>`.
    pub fn title_key(self) -> String {
        format!("commands.group.{}", self.id())
    }
}

/// How a command is reached over MCP (the coverage table, RB-030-08).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpCoverage {
    /// An MCP tool covers it (`covered(view_set_breakpoint)`).
    Covered(String),
    /// No tool, with the reason (`omitted("ui-only: key recording")`).
    Omitted(String),
}

/// A command's definition. Every command has at least one default binding, and one of them is
/// a leader chord, so every command is reachable through the leader (coverage test,
/// RB-010-17).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandDef {
    /// The id.
    pub id: CommandId,
    /// The i18n key of the title (`commands.<id>`).
    pub title_key: String,
    /// The leader group.
    pub group: Group,
    /// The default bindings; the user's bindings (user settings `shortcuts.bindings`) replace
    /// them.
    pub default_bindings: Vec<Binding>,
    /// The MCP mapping.
    pub mcp: McpCoverage,
}

/// What runs a command: JSON arguments in, a JSON result out. Handlers capture what they need
/// (the `ViewHost`, the config) and must not block the UI thread.
pub type Handler = Arc<dyn Fn(Value) -> Result<Value, CommandError> + Send + Sync>;

/// Why a command could not be registered, bound or run.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CommandError {
    /// The contract stub: RB-010-17 has not landed yet.
    #[error("not implemented yet (contract 0.1 stub)")]
    NotImplemented,
    /// The id breaks the format.
    #[error("invalid command id `{0}`")]
    InvalidId(String),
    /// No command has this id.
    #[error("unknown command `{0}`")]
    UnknownCommand(String),
    /// The id is registered already.
    #[error("command `{0}` registered twice")]
    Duplicate(String),
    /// A binding's text form does not parse.
    #[error("invalid binding `{0}`")]
    InvalidBinding(String),
    /// The binding is on the reservation list; the key is the i18n key of the reason.
    #[error("reserved: {0}")]
    Reserved(String),
    /// Another command has the binding.
    #[error("`{binding}` is bound to `{other}` already")]
    Conflict {
        /// The binding text.
        binding: String,
        /// The command holding it.
        other: String,
    },
    /// The arguments do not fit the command.
    #[error("invalid arguments: {0}")]
    InvalidArgs(String),
    /// The command ran and failed.
    #[error("command failed: {0}")]
    Failed(String),
    /// The reservation list in the binary does not parse.
    #[error("reserved-shortcuts.json: {0}")]
    ReservedList(String),
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
