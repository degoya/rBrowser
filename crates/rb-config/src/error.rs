//! What loading, validating and migrating a config reports (contract 0.1; RB-010-01 fills the
//! variants). The `key` of an issue is an i18n key under `config.error.*` / `config.warning.*`
//! (RB-010-14); `message` is the English fallback for logs.

use std::path::PathBuf;

/// One validation problem or warning, located by a JSON pointer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigIssue {
    /// JSON pointer into the file (`/breakpoints/1/id`).
    pub path: String,
    /// The i18n key (`config.error.duplicate_id`, `config.warning.local_only_ignored`).
    pub key: String,
    /// English text for logs and tests.
    pub message: String,
}

/// A note of what a migration changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MigrationNote {
    /// The version migrated from.
    pub from: u32,
    /// The version migrated to.
    pub to: u32,
    /// The i18n key of the note (`config.migration.*`).
    pub key: String,
    /// English text.
    pub message: String,
}

/// Why a config could not be loaded.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The contract stub: RB-010-01 has not landed yet.
    #[error("not implemented yet (contract 0.1 stub)")]
    NotImplemented,
    /// Reading or writing a file failed.
    #[error("cannot access {path}: {source}")]
    Io {
        /// The file.
        path: PathBuf,
        /// The cause.
        #[source]
        source: std::io::Error,
    },
    /// The file is not valid JSON.
    #[error("{path}:{line}:{column}: {message}")]
    Parse {
        /// The file.
        path: PathBuf,
        /// 1-based line.
        line: usize,
        /// 1-based column.
        column: usize,
        /// The parser's message.
        message: String,
    },
    /// The file is JSON but breaks the schema or a rule (duplicate id, cell outside the grid).
    #[error("invalid config: {} problem(s)", .0.len())]
    Invalid(Vec<ConfigIssue>),
    /// The file's `version` is newer than this build knows.
    #[error("config version {found} is newer than {supported}")]
    UnsupportedVersion {
        /// The version in the file.
        found: u32,
        /// [`crate::CURRENT_VERSION`].
        supported: u32,
    },
}

impl ConfigError {
    /// The i18n key that names the kind of error for the UI; an [`ConfigError::Invalid`] error
    /// shows the keys of its issues beside it.
    pub fn key(&self) -> &'static str {
        match self {
            ConfigError::NotImplemented => "config.error.not_implemented",
            ConfigError::Io { .. } => "config.error.io",
            ConfigError::Parse { .. } => "config.error.parse",
            ConfigError::Invalid(_) => "config.error.invalid",
            ConfigError::UnsupportedVersion { .. } => "config.error.unsupported_version",
        }
    }
}
