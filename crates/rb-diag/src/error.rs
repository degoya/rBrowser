//! What can go wrong in logging and diagnostics, each with the i18n key the shell shows and,
//! where one helps, the setting to link to.

use std::io;
use std::path::PathBuf;

/// An error of logging or of the diagnostics ZIP.
#[derive(Debug, thiserror::Error)]
pub enum DiagError {
    /// The machine has no local app data directory (`%LOCALAPPDATA%` unset).
    #[error("no directory for logs: LOCALAPPDATA is not set")]
    NoLogDir,
    /// The log directory or file cannot be written.
    #[error("cannot write the log in {path}: {source}")]
    Log {
        /// The directory.
        path: PathBuf,
        /// The cause.
        source: io::Error,
    },
    /// The process has a logger already.
    #[error("a logger is installed already")]
    AlreadyInstalled,
    /// The log filter cannot be changed.
    #[error("the log level cannot be changed: {0}")]
    Filter(String),
    /// The diagnostics ZIP cannot be written.
    #[error("cannot write the diagnostics ZIP {path}: {source}")]
    Bundle {
        /// The ZIP's path.
        path: PathBuf,
        /// The cause.
        source: io::Error,
    },
}

impl DiagError {
    /// The i18n key of the user-facing text (`locales/en.json`); placeholder
    /// `{path}` where the error has one.
    pub fn i18n_key(&self) -> &'static str {
        match self {
            DiagError::NoLogDir => "diagnostics.error.noLogDir",
            DiagError::Log { .. } => "diagnostics.error.logNotWritable",
            DiagError::AlreadyInstalled => "diagnostics.error.alreadyInstalled",
            DiagError::Filter(_) => "diagnostics.error.levelNotChanged",
            DiagError::Bundle { .. } => "diagnostics.error.bundleNotWritable",
        }
    }

    /// The setting a deep link in the error opens, where one helps.
    pub fn settings_key(&self) -> Option<&'static str> {
        match self {
            DiagError::Filter(_) => Some(crate::settings::KEY_LEVEL),
            _ => None,
        }
    }
}
