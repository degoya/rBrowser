//! Why a manifest, a check or a download was refused: [`UpdateError::code`] is the stable code the
//! window translates (`update.error.<code>`, RB-090-14) and the log names.
//!
//! Copied and adapted from rCommander's `crates/rc-update/src/error.rs` (RB-090-02): `disabled`
//! for the switched-off feature, no proxy login.

use crate::{manifest::Channel, manifest::MAX_MANIFEST_BYTES, sign::replay::StaleError};

/// Why a manifest, a check or a download was refused.
#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("self-update is switched off; nothing was fetched")]
    Disabled,
    #[error("this build carries no update signing key; the update check is not configured")]
    NotConfigured,
    #[error("no update manifest is published for this channel yet")]
    NotPublished,
    #[error("the update manifest could not be fetched: {0}")]
    Fetch(String),
    #[error("the update manifest is larger than {MAX_MANIFEST_BYTES} bytes")]
    TooLarge,
    #[error("the update manifest cannot be read: {0}")]
    Malformed(String),
    #[error("the update manifest is not signed by a trusted update key: {0}")]
    Untrusted(String),
    #[error("the update manifest's signature does not verify: {0}")]
    BadSignature(String),
    #[error("the update manifest has been revoked")]
    Revoked,
    #[error("the update manifest declares schema version {saw}, this build reads {expected}")]
    SchemaVersion { saw: u64, expected: u32 },
    #[error("the update manifest is for the {saw} channel, not {expected}")]
    WrongChannel { expected: Channel, saw: Channel },
    #[error("the update manifest is stale: {0}")]
    Stale(#[from] StaleError),
    #[error("the update manifest is invalid: {0}")]
    Invalid(String),
    #[error("the download failed: {0}")]
    Download(String),
    #[error("the download is {saw} bytes, the manifest says {expected}")]
    SizeMismatch { expected: u64, saw: u64 },
    #[error("the downloaded file is not the one the manifest describes")]
    DigestMismatch,
    #[error("{free} bytes are free where the update goes, it needs {required}")]
    NoSpace { free: u64, required: u64 },
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Every code [`UpdateError::code`] answers.
pub const UPDATE_ERROR_CODES: &[&str] = &[
    "update.disabled",
    "update.not_configured",
    "update.not_published",
    "update.fetch_failed",
    "update.too_large",
    "update.malformed",
    "update.untrusted",
    "update.bad_signature",
    "update.revoked",
    "update.schema_version_unsupported",
    "update.wrong_channel",
    "update.stale",
    "update.invalid",
    "update.download_failed",
    "update.size_mismatch",
    "update.digest_mismatch",
    "update.no_space",
    "update.failed",
];

impl UpdateError {
    /// Whether this refuses what a source served - a forged, replayed, misplaced or malformed
    /// manifest - rather than says that nothing could be reached: the one worth reporting
    /// louder when another manifest of the same check verified. An expired manifest is none of
    /// these ([`Self::is_expired`]): a channel without a release for its validity is simply
    /// nothing to offer.
    #[must_use]
    pub fn is_integrity(&self) -> bool {
        match self {
            Self::Stale(stale) => !matches!(stale, StaleError::Expired { .. }),
            Self::TooLarge
            | Self::Malformed(_)
            | Self::Untrusted(_)
            | Self::BadSignature(_)
            | Self::Revoked
            | Self::SchemaVersion { .. }
            | Self::WrongChannel { .. }
            | Self::Invalid(_) => true,
            _ => false,
        }
    }

    /// Whether the manifest is past its `not_after`: nothing to offer, worth a log line only
    /// (180 days without a release, or a beta channel after the last beta).
    #[must_use]
    pub fn is_expired(&self) -> bool {
        matches!(self, Self::Stale(StaleError::Expired { .. }))
    }

    /// The stable code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Disabled => "update.disabled",
            Self::NotConfigured => "update.not_configured",
            Self::NotPublished => "update.not_published",
            Self::Fetch(_) => "update.fetch_failed",
            Self::TooLarge => "update.too_large",
            Self::Malformed(_) => "update.malformed",
            Self::Untrusted(_) => "update.untrusted",
            Self::BadSignature(_) => "update.bad_signature",
            Self::Revoked => "update.revoked",
            Self::SchemaVersion { .. } => "update.schema_version_unsupported",
            Self::WrongChannel { .. } => "update.wrong_channel",
            Self::Stale(_) => "update.stale",
            Self::Invalid(_) => "update.invalid",
            Self::Download(_) => "update.download_failed",
            Self::SizeMismatch { .. } => "update.size_mismatch",
            Self::DigestMismatch => "update.digest_mismatch",
            Self::NoSpace { .. } => "update.no_space",
            Self::Other(_) => "update.failed",
        }
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
