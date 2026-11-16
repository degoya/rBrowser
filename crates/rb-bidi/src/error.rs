//! [`BidiError`]: why a BiDi command, the launch or the connection failed. No variant carries a
//! command's parameters (a replayed password travels in them) or a frame Firefox sent.

use crate::protocol::Failure;

/// Why talking to Firefox failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BidiError {
    /// The WebSocket could not be opened.
    #[error("cannot connect to Firefox's WebDriver BiDi endpoint: {0}")]
    Connect(String),
    /// The endpoint is not on this machine; only loopback endpoints are dialled.
    #[error("Firefox's WebDriver BiDi endpoint is not a loopback address")]
    NotLoopback,
    /// The connection ended (Firefox quit, crashed or closed the socket).
    #[error("the connection to Firefox is closed")]
    Closed,
    /// No answer in time.
    #[error("Firefox did not answer {method} within {ms} ms")]
    Timeout {
        /// The command.
        method: String,
        /// How long it waited.
        ms: u64,
    },
    /// Firefox answered with an error.
    #[error("Firefox refused {method} ({error}): {message}")]
    Command {
        /// The command.
        method: String,
        /// BiDi's error code.
        error: String,
        /// Firefox's explanation.
        message: String,
    },
    /// The answer was not what the command returns.
    #[error("Firefox's answer to {method} is not understood: {reason}")]
    Protocol {
        /// The command.
        method: String,
        /// What was wrong (never the answer itself).
        reason: String,
    },
    /// Firefox could not be found or started.
    #[error("{0}")]
    Launch(String),
    /// The installed Firefox is too old for rBrowser.
    #[error("Firefox {found} is too old; rBrowser needs Firefox {min} or newer")]
    TooOld {
        /// The version found.
        found: String,
        /// The oldest major version that works.
        min: u32,
    },
}

impl BidiError {
    /// `method` was refused with `failure`.
    pub fn refused(method: &str, failure: Failure) -> Self {
        Self::Command {
            method: method.to_owned(),
            error: failure.error,
            message: failure.message,
        }
    }

    /// Firefox does not know the command or cannot do it: the feature is not there, which is no
    /// failure of the view.
    pub fn is_unsupported(&self) -> bool {
        matches!(
            self,
            Self::Command { error, .. }
                if error == "unknown command" || error == "unsupported operation"
        )
    }
}
