//! The user settings of WSL: the `wsl` section of the user settings file. The schema of the keys
//! is `schema/parts/wsl.json` (Reload & Files > WSL watcher, Automation > CDP proxy).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::netmode::NetworkingMode;

/// `wsl.networkingModeOverride`: the detected mode, or one set by hand.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModeOverride {
    /// Detect it (`wslinfo`, `.wslconfig`, NAT).
    #[default]
    Auto,
    /// Treat every distribution as mirrored.
    Mirrored,
    /// Treat every distribution as NAT.
    Nat,
}

impl ModeOverride {
    /// The mode set by hand; `None` for [`Self::Auto`].
    #[must_use]
    pub fn mode(self) -> Option<NetworkingMode> {
        match self {
            Self::Auto => None,
            Self::Mirrored => Some(NetworkingMode::Mirrored),
            Self::Nat => Some(NetworkingMode::Nat),
        }
    }
}

/// `wsl`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WslSettings {
    /// `wsl.helperPath`: another `rbrowser-wsl` than the packaged one (Linux or Windows path).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub helper_path: Option<String>,
    /// `wsl.distro`: the distribution to use instead of the project's or WSL's default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distro: Option<String>,
    /// `wsl.networkingModeOverride`.
    pub networking_mode_override: ModeOverride,
}

impl WslSettings {
    /// The settings in the `wsl` section `value`; a section that does not read (a value of the
    /// wrong type, written by hand) gives the defaults, with a line in the log.
    #[must_use]
    pub fn from_value(value: Option<&Value>) -> Self {
        let Some(value) = value else {
            return Self::default();
        };
        serde_json::from_value(value.clone()).unwrap_or_else(|error| {
            tracing::warn!(%error, "the wsl settings do not read; the defaults apply");
            Self::default()
        })
    }

    /// `wsl.helperPath` when it names something.
    #[must_use]
    pub fn helper_path(&self) -> Option<&str> {
        non_blank(self.helper_path.as_deref())
    }

    /// `wsl.distro` when it names something.
    #[must_use]
    pub fn distro(&self) -> Option<&str> {
        non_blank(self.distro.as_deref())
    }
}

fn non_blank(text: Option<&str>) -> Option<&str> {
    text.map(str::trim).filter(|text| !text.is_empty())
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
