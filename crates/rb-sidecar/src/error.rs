//! What can go wrong with the sidecar, each with the i18n key the shell shows, its placeholders
//! and, where one helps, the setting a deep link opens.

use std::io;
use std::path::PathBuf;

use serde_json::Value;

use crate::settings::{KEY_BROWSERS_PATH, KEY_NODE_PATH, KEY_RPC_TIMEOUT};

/// An error of the sidecar: finding Node, starting it, or a request.
#[derive(Debug, thiserror::Error)]
pub enum SidecarError {
    /// Neither `engines.nodePath` nor the bundled runtime.
    #[error("no Node runtime: {0} does not exist and engines.nodePath is empty")]
    NodeNotFound(PathBuf),
    /// `engines.nodePath` is no absolute path to a file.
    #[error("engines.nodePath {0} is not an absolute path to a file")]
    NodePathInvalid(PathBuf),
    /// The runtime's major version differs from `.nvmrc`.
    #[error("Node {expected} expected, {found} found ({path})")]
    NodeVersionMismatch {
        /// The major version of `.nvmrc`.
        expected: u32,
        /// What `node --version` printed, without the `v`.
        found: String,
        /// The runtime.
        path: PathBuf,
    },
    /// `node --version` failed or printed no version.
    #[error("the version of {path} cannot be read: {reason}")]
    NodeVersionUnknown {
        /// The runtime.
        path: PathBuf,
        /// Why.
        reason: String,
    },
    /// `sidecar/dist/main.js` is missing beside the executable.
    #[error("the sidecar script {0} is missing")]
    ScriptNotFound(PathBuf),
    /// The process cannot be started.
    #[error("the sidecar cannot be started: {0}")]
    Spawn(#[source] io::Error),
    /// The process started but did not answer `sidecar.ping`.
    #[error("the sidecar did not answer its health ping: {0}")]
    Unhealthy(String),
    /// The process ended while the request ran, or before it was sent.
    #[error("the sidecar exited")]
    Exited,
    /// The process crashed or failed to start too often in a row; only `sidecar.restart` starts
    /// it again.
    #[error(
        "the sidecar crashed or failed to start {crashes} times in a row and is not restarted automatically"
    )]
    CrashLoop {
        /// The crashes in a row.
        crashes: u32,
    },
    /// rBrowser quits: the sidecar was shut down and starts no more.
    #[error("the sidecar was shut down")]
    ShutDown,
    /// No answer within the timeout (`engines.nodeRpcTimeoutMs`); the request was cancelled.
    #[error("`{method}` got no answer within {ms} ms")]
    Timeout {
        /// The method.
        method: String,
        /// The timeout.
        ms: u64,
    },
    /// The sidecar answered with an error.
    #[error("`{method}` failed ({code}): {message}")]
    Remote {
        /// The method.
        method: String,
        /// The JSON-RPC error code.
        code: i64,
        /// The sidecar's message.
        message: String,
        /// The sidecar's data, if any (boxed: it keeps every `Result` of the crate small).
        data: Option<Box<Value>>,
    },
    /// A message that breaks the protocol.
    #[error("sidecar protocol error: {0}")]
    Protocol(String),
    /// No directory for the Playwright browsers: the setting is empty and `LOCALAPPDATA` unset,
    /// or the setting is no absolute path.
    #[error("no directory for the Playwright browsers: {0}")]
    BrowsersPath(String),
}

impl SidecarError {
    /// The i18n key of the user-facing text (`locales/parts/RB-040-03.json`); its placeholders
    /// are [`SidecarError::i18n_args`].
    pub fn i18n_key(&self) -> &'static str {
        match self {
            SidecarError::NodeNotFound(_) => "sidecar.error.nodeNotFound",
            SidecarError::NodePathInvalid(_) => "sidecar.error.nodePathInvalid",
            SidecarError::NodeVersionMismatch { .. } => "sidecar.error.nodeVersionMismatch",
            SidecarError::NodeVersionUnknown { .. } => "sidecar.error.nodeVersionUnknown",
            SidecarError::ScriptNotFound(_) => "sidecar.error.scriptNotFound",
            SidecarError::Spawn(_) => "sidecar.error.spawn",
            SidecarError::Unhealthy(_) => "sidecar.error.unhealthy",
            SidecarError::Exited => "sidecar.error.exited",
            SidecarError::CrashLoop { .. } => "sidecar.error.crashLoop",
            SidecarError::ShutDown => "sidecar.error.shutDown",
            SidecarError::Timeout { .. } => "sidecar.error.timeout",
            SidecarError::Remote { .. } => "sidecar.error.remote",
            SidecarError::Protocol(_) => "sidecar.error.protocol",
            SidecarError::BrowsersPath(_) => "sidecar.error.browsersPath",
        }
    }

    /// The placeholders of [`SidecarError::i18n_key`]'s text.
    pub fn i18n_args(&self) -> Vec<(&'static str, String)> {
        match self {
            SidecarError::NodeNotFound(path)
            | SidecarError::NodePathInvalid(path)
            | SidecarError::ScriptNotFound(path) => vec![("path", path.display().to_string())],
            SidecarError::NodeVersionMismatch {
                expected, found, ..
            } => vec![("expected", expected.to_string()), ("found", found.clone())],
            SidecarError::NodeVersionUnknown { path, .. } => {
                vec![("path", path.display().to_string())]
            }
            SidecarError::CrashLoop { crashes } => vec![("crashes", crashes.to_string())],
            SidecarError::Timeout { method, ms } => {
                vec![("method", method.clone()), ("ms", ms.to_string())]
            }
            SidecarError::Remote {
                method, message, ..
            } => vec![
                ("method", method.clone()),
                ("message", rb_diag::redact(message)),
            ],
            _ => Vec::new(),
        }
    }

    /// The setting a deep link in the error opens, where one helps.
    pub fn settings_key(&self) -> Option<&'static str> {
        match self {
            SidecarError::NodeNotFound(_)
            | SidecarError::NodePathInvalid(_)
            | SidecarError::NodeVersionMismatch { .. }
            | SidecarError::NodeVersionUnknown { .. } => Some(KEY_NODE_PATH),
            SidecarError::Timeout { .. } => Some(KEY_RPC_TIMEOUT),
            SidecarError::BrowsersPath(_) => Some(KEY_BROWSERS_PATH),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
