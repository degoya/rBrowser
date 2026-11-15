//! `updates`: the self-update keys of the user settings (RB-090-02), moved out of
//! `user_settings` and re-exported there.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `updates`. Self-update is prepared and inactive until the move to GitHub (RB-090-02,
/// RB-110-05): without the app's Cargo feature `self-update` the window shows every key but
/// `cefAgeWarning` locked, and a write is refused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct UpdateSettings {
    /// Self-update at all: off, nothing is fetched and nothing installed (RB-090-02).
    pub enabled: bool,
    /// The update channel.
    pub channel: UpdateChannel,
    /// When to look for an update (RB-090-02).
    pub check: UpdateCheck,
    /// When a downloaded update installs by itself (RB-090-02).
    pub auto_install: UpdateAutoInstall,
    /// A notice when the bundled CEF is older than six weeks (RB-010-10).
    pub cef_age_warning: bool,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            channel: UpdateChannel::default(),
            check: UpdateCheck::default(),
            auto_install: UpdateAutoInstall::default(),
            cef_age_warning: true,
        }
    }
}

/// When to look for an update.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum UpdateCheck {
    /// Never by itself.
    #[default]
    Off,
    /// Once at every start.
    OnStart,
}

/// When a downloaded update installs by itself.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum UpdateAutoInstall {
    /// Only when a person asks.
    #[default]
    Off,
    /// When rBrowser quits.
    OnQuit,
}

/// An update channel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    /// Releases.
    #[default]
    Stable,
    /// Pre-releases.
    Beta,
}
