//! The answer of the MCP tool `wsl_info` (command `wsl.info`; `action: "restart"` is the
//! command `wsl.restart_helper`): distributions, the one in use, its networking mode and the
//! helper's state. The tool itself is registered by rb-mcp (RB-030-01).

use serde::{Deserialize, Serialize};

use crate::distro::Distro;
use crate::netmode::Detection;
use crate::supervisor::HelperStatus;

/// What `wsl_info` is asked to do.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WslInfoAction {
    /// Report.
    #[default]
    Info,
    /// Restart the helper, then report.
    Restart,
}

/// The arguments of `wsl_info`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WslInfoArgs {
    /// What to do; `info` by default.
    #[serde(default)]
    pub action: WslInfoAction,
}

/// A failure as `wsl_info` reports it: the key a client can translate and the English text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InfoError {
    /// The i18n key.
    pub key: String,
    /// The English message.
    pub message: String,
}

impl From<&crate::WslError> for InfoError {
    fn from(error: &crate::WslError) -> Self {
        Self {
            key: error.i18n_key().to_owned(),
            message: error.to_string(),
        }
    }
}

/// The answer of `wsl_info`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WslInfo {
    /// The installed distributions; empty when WSL is not available.
    pub distros: Vec<Distro>,
    /// The distribution in use.
    pub distro: Option<String>,
    /// Its networking mode and where it was read.
    pub networking: Option<Detection>,
    /// Whether `127.0.0.1` in the distribution reaches rBrowser on Windows.
    pub reaches_windows_localhost: Option<bool>,
    /// The helper's state.
    pub helper: HelperStatus,
    /// Why WSL is not available, when it is not.
    pub error: Option<InfoError>,
}

impl WslInfo {
    /// The answer from its parts.
    #[must_use]
    pub fn new(
        distros: Vec<Distro>,
        distro: Option<String>,
        networking: Option<Detection>,
        helper: HelperStatus,
        error: Option<&crate::WslError>,
    ) -> Self {
        Self {
            reaches_windows_localhost: networking.map(|d| d.mode.reaches_windows_localhost()),
            distros,
            distro,
            networking,
            helper,
            error: error.map(InfoError::from),
        }
    }
}

#[cfg(test)]
#[path = "info_tests.rs"]
mod tests;
