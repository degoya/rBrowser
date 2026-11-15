//! What a manifest's `[capabilities]` may grant (`docs/plugins.md`, "Capabilities and rights").
//!
//! New in rBrowser: rCommander grants per plugin kind and one network capability; here every host
//! interface beyond `host` and `settings` is one capability, and a component importing an
//! interface whose capability is not granted is refused at install and at load. Logging is always
//! there, capped by the log budget.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::{Failure, FailureKind, codes};

/// One right a manifest can grant. The id is the manifest's key (`project-read = { ... }`).
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    /// Reading the views and their state.
    Views,
    /// Running a script in a view's isolated world.
    Evaluate,
    /// CDP commands of the domains the manifest lists.
    Cdp,
    /// HTTP fetch, rate-limited.
    Http,
    /// Reading `.rbrowser/` (without `config.local.json`) and the declared globs.
    ProjectRead,
    /// Writing into `.rbrowser/out/<plugin-id>/` only.
    OutWrite,
    /// Reporting findings to the shared findings list.
    Findings,
    /// Calling the Node sidecar (Lighthouse, Playwright).
    Sidecar,
    /// Supplying declarative interception rules; the host applies them, no import.
    NetworkRules,
    /// Receiving the views' console, error and network buffers as the argument `$console` of
    /// every call (Charge 6); no import - the host hands them over.
    ConsoleLogs,
    /// Short notices in the status bar, the plugin's own texts under its name (`status` posts,
    /// RB-080-64; review S1); no import.
    Status,
}

/// A capability id rBrowser does not know.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("unknown capability `{0}`")]
pub struct UnknownCapability(pub String);

impl Capability {
    /// Every capability, in the order of `docs/plugins.md`.
    pub const ALL: [Self; 11] = [
        Self::Views,
        Self::Evaluate,
        Self::Cdp,
        Self::Http,
        Self::ProjectRead,
        Self::OutWrite,
        Self::Findings,
        Self::Sidecar,
        Self::NetworkRules,
        Self::ConsoleLogs,
        Self::Status,
    ];

    /// The id a manifest names it by.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Views => "views",
            Self::Evaluate => "evaluate",
            Self::Cdp => "cdp",
            Self::Http => "http",
            Self::ProjectRead => "project-read",
            Self::OutWrite => "out-write",
            Self::Findings => "findings",
            Self::Sidecar => "sidecar",
            Self::NetworkRules => "network-rules",
            Self::ConsoleLogs => "console-logs",
            Self::Status => "status",
        }
    }

    /// The WIT interface of `rbrowser:plugin` it grants; `None` for one without an import.
    #[must_use]
    pub const fn interface(self) -> Option<&'static str> {
        match self {
            Self::Views => Some("views"),
            Self::Evaluate => Some("evaluate"),
            Self::Cdp => Some("cdp"),
            Self::Http => Some("http"),
            Self::ProjectRead => Some("project"),
            Self::OutWrite => Some("out"),
            Self::Findings => Some("findings"),
            Self::Sidecar => Some("sidecar"),
            Self::NetworkRules | Self::ConsoleLogs | Self::Status => None,
        }
    }

    /// The capability granting the WIT interface `interface`, if one does.
    #[must_use]
    pub fn for_interface(interface: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|capability| capability.interface() == Some(interface))
    }

    /// What a plugin gets for a host function its manifest does not grant:
    /// `plugins.error.capability_denied` naming the capability.
    #[must_use]
    pub fn denied(self) -> Failure {
        Failure::coded(FailureKind::AccessDenied, codes::CAPABILITY_DENIED)
            .with_param("capability", self.id())
    }

    /// The i18n key of its one-line description, shown in the install preview and the trust
    /// prompt (`plugins.capability.<id>`).
    #[must_use]
    pub fn description_key(self) -> String {
        format!("plugins.capability.{}", self.id())
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.id())
    }
}

impl FromStr for Capability {
    type Err = UnknownCapability;

    fn from_str(id: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|capability| capability.id() == id)
            .ok_or_else(|| UnknownCapability(id.to_owned()))
    }
}

#[cfg(test)]
#[path = "capability_tests.rs"]
mod tests;
