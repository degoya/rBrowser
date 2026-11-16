//! The plugin kinds: one WIT world each (`docs/plugins.md`, "Plugin kinds").
//!
//! A plugin may implement more than one world: its component targets a world of its own that
//! `include`s them, and its manifest lists every kind (`kinds = ["analyzer-plugin",
//! "feature-plugin"]`). Commands and MCP tools are served by the `feature` export, so a plugin
//! that declares either is a `feature-plugin` too.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// One world of `rbrowser:plugin`. Serialized as the world's name.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum PluginKind {
    /// Commands, MCP tools, panels, overlays, page scripts.
    #[serde(rename = "feature-plugin")]
    Feature,
    /// Findings in one format.
    #[serde(rename = "check-plugin")]
    Check,
    /// Proposes config changes (breakpoints).
    #[serde(rename = "analyzer-plugin")]
    Analyzer,
    /// Recorder to code.
    #[serde(rename = "export-plugin")]
    Export,
}

/// A kind rBrowser does not know.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("unknown plugin kind `{0}`")]
pub struct UnknownKind(pub String);

impl PluginKind {
    /// Every kind, in the order of the contract.
    pub const ALL: [Self; 4] = [Self::Feature, Self::Check, Self::Analyzer, Self::Export];

    /// The WIT world's name, which the manifest's `kinds` uses.
    #[must_use]
    pub const fn world(self) -> &'static str {
        match self {
            Self::Feature => "feature-plugin",
            Self::Check => "check-plugin",
            Self::Analyzer => "analyzer-plugin",
            Self::Export => "export-plugin",
        }
    }

    /// The interface the world exports.
    #[must_use]
    pub const fn export(self) -> &'static str {
        match self {
            Self::Feature => "feature",
            Self::Check => "check",
            Self::Analyzer => "analyzer",
            Self::Export => "exporter",
        }
    }

    /// The SDK template folder (`sdk/templates/<name>`).
    #[must_use]
    pub const fn template(self) -> &'static str {
        match self {
            Self::Feature => "feature",
            Self::Check => "check",
            Self::Analyzer => "analyzer",
            Self::Export => "export",
        }
    }
}

impl fmt::Display for PluginKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.world())
    }
}

impl FromStr for PluginKind {
    type Err = UnknownKind;

    fn from_str(world: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.world() == world)
            .ok_or_else(|| UnknownKind(world.to_owned()))
    }
}
