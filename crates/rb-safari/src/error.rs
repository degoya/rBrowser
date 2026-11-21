//! [`SafariError`]: why a WebDriver command, the driver's start or the connection failed. No
//! variant carries a command's body (a replayed password travels in it) or a page's answer.

use rb_engine::{EngineError, EngineKind};

/// Why talking to `safaridriver` failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SafariError {
    /// Nobody answers on the driver's port: it ended or never started.
    #[error("safaridriver does not answer: {0}")]
    Connect(String),
    /// No answer in time.
    #[error("safaridriver did not answer {command} within {ms} ms")]
    Timeout {
        /// The command.
        command: String,
        /// How long it waited.
        ms: u64,
    },
    /// The driver answered with a WebDriver error.
    #[error("Safari refused {command} ({error}): {message}")]
    Command {
        /// The command.
        command: String,
        /// WebDriver's error code (`no such element`, `session not created`).
        error: String,
        /// The driver's explanation, cut short.
        message: String,
    },
    /// The answer was not what the command returns.
    #[error("safaridriver's answer to {command} is not understood: {reason}")]
    Protocol {
        /// The command.
        command: String,
        /// What was wrong (never the answer itself).
        reason: String,
    },
    /// Remote automation is off: `safaridriver --enable` never ran, or Safari's Develop menu
    /// does not allow remote automation.
    #[error("Safari does not allow remote automation")]
    NotEnabled,
    /// Safari runs another automated session already.
    #[error("Safari runs another automated session")]
    SessionBusy,
    /// `safaridriver` could not be found or started.
    #[error("{0}")]
    Launch(String),
}

impl SafariError {
    /// Whether the driver is gone: every view of it ends as crashed.
    pub fn driver_gone(&self) -> bool {
        matches!(self, Self::Connect(_))
    }

    /// Whether the driver does not know the command (an older Safari): the feature is missing,
    /// not the view.
    pub fn unsupported(&self) -> bool {
        matches!(
            self,
            Self::Command { error, .. }
                if error == "unknown command" || error == "unsupported operation"
                    || error == "unknown method"
        )
    }
}

/// The engine error of a Safari error; the reasons a person reads come from the catalogue.
pub fn engine_error(error: SafariError) -> EngineError {
    engine_error_for(EngineKind::Safari, error)
}

/// [`engine_error`] for the engine `engine` (Safari or iOS, RB-095-04): iOS names the switch on
/// the device or simulator (Settings > Safari > Advanced > Remote Automation).
pub fn engine_error_for(engine: EngineKind, error: SafariError) -> EngineError {
    let ios = engine == EngineKind::Ios;
    match error {
        SafariError::Connect(_) => EngineError::Closed,
        SafariError::NotEnabled => EngineError::Unavailable {
            engine,
            reason: rb_i18n::t(if ios {
                "engines.ios.notEnabled"
            } else {
                "engines.safari.notEnabled"
            }),
        },
        SafariError::SessionBusy => EngineError::Unavailable {
            engine,
            reason: rb_i18n::t(if ios {
                "engines.ios.oneSession"
            } else {
                "engines.safari.oneSession"
            }),
        },
        SafariError::Launch(reason) => EngineError::Unavailable { engine, reason },
        ref unsupported if unsupported.unsupported() => EngineError::Unsupported {
            engine,
            feature: unsupported.to_string(),
        },
        other => EngineError::Invalid(other.to_string()),
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
