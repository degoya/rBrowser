//! What goes wrong with WSL, each with the i18n key of the message a user reads.

use crate::path::PathError;
use crate::protocol::HandshakeError;

/// A WSL failure the app shows.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WslError {
    /// `wsl.exe` is missing, or WSL is switched off.
    #[error("WSL is not installed or switched off")]
    NotInstalled,
    /// WSL runs but has no distribution.
    #[error("no WSL distribution is installed")]
    NoDistro,
    /// The distribution asked for is not installed.
    #[error("the WSL distribution {0:?} is not installed")]
    DistroNotFound(String),
    /// `wsl.exe` did not answer in time and was stopped.
    #[error("wsl.exe did not answer within {0} ms")]
    Timeout(u64),
    /// `wsl.exe` ended with a failure; `detail` is the first line it wrote to stderr.
    #[error("wsl.exe failed ({status}): {detail}")]
    Failed {
        /// The exit status as text.
        status: String,
        /// The first line of its error output, shortened.
        detail: String,
    },
    /// A path without a counterpart.
    #[error(transparent)]
    Path(#[from] PathError),
    /// The helper answered the handshake wrongly.
    #[error(transparent)]
    Handshake(#[from] HandshakeError),
    /// The helper ended before its handshake: not found in the distribution (no drive mounts),
    /// or not executable there.
    #[error("the WSL helper could not be started: {0}")]
    HelperStart(String),
    /// The helper crashed too often in a row and is no longer restarted.
    #[error("the WSL helper stopped {0} times in a row and is no longer restarted")]
    HelperCrashed(u32),
}

impl WslError {
    /// The i18n key of the message a user reads.
    #[must_use]
    pub fn i18n_key(&self) -> &'static str {
        match self {
            Self::NotInstalled => "wsl.error.notInstalled",
            Self::NoDistro => "wsl.error.noDistro",
            Self::DistroNotFound(_) => "wsl.error.distroNotFound",
            Self::Timeout(_) => "wsl.error.timeout",
            Self::Failed { .. } => "wsl.error.failed",
            Self::Path(error) => error.i18n_key(),
            Self::Handshake(error) => error.i18n_key(),
            Self::HelperStart(_) => "wsl.error.helperStart",
            Self::HelperCrashed(_) => "wsl.error.helperCrashed",
        }
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
