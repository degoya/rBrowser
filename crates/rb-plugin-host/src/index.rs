//! The signed plugin index `rbrowser-plugin-index.json` (RB-091-01): what a plugin repository
//! offers.
//!
//! Adapted from rDownloader's `crates/rd-plugin-host/src/index.rs` (RD-140-01). An index is a
//! [`SignedDocument`] under [`PLUGIN_INDEX_DOMAIN`], signed by the repository's key - for the
//! official catalogue the plugin release key compiled into the app (`sign::EMBEDDED_KEYS`), for
//! any other the key the person entered with its address. **A repository only delivers; it
//! vouches for nothing**: every package it lists is verified again when it is installed, exactly
//! as a `.rbplugin` file is (signature, trusted key, capabilities), after its bytes matched the
//! index's SHA-256 and size.
//!
//! Order matters: size, envelope, signature, schema version, freshness (sequence, expiry, issue
//! time), content. An index that fails at any step yields no entry at all. The schema version is
//! read *after* the signature, so an unsigned document declaring a future version is reported as
//! unsigned. Not taken from rDownloader: revocations inside the index and the carried-over
//! entries of earlier indexes - an index here lists what it offers now.

use chrono::{DateTime, Utc};
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};

#[path = "index_build.rs"]
mod build;
#[cfg(test)]
#[path = "index_fixtures.rs"]
pub(crate) mod fixtures;
#[path = "index_merge.rs"]
mod merge;
#[path = "index_sign.rs"]
mod signed;
#[path = "index_validate.rs"]
mod validate;

pub use build::{describe_package, package_file_name};
pub use merge::{CatalogEntry, Compatibility, Offer, compatibility, merge_offers, update_for};
pub use signed::{
    ALGORITHM_ED25519, DocumentSignature, Freshness, IndexSigners, MAX_CLOCK_SKEW_MINUTES,
    SignatureError, SignedDocument, StaleError, check_freshness, sign_document,
};

/// Domain separator of the index's signatures.
pub const PLUGIN_INDEX_DOMAIN: &str = "rbrowser.plugin-index.v1";
/// The file name an index is published under.
pub const INDEX_FILE_NAME: &str = "rbrowser-plugin-index.json";
/// The index layout this build reads; anything else is refused as a whole.
pub const PLUGIN_INDEX_SCHEMA_VERSION: u32 = 1;
/// Largest signed index accepted, checked before a byte of it is parsed.
pub const MAX_INDEX_BYTES: usize = 4 * 1024 * 1024;
/// Most plugins one index may list.
pub const MAX_INDEX_PLUGINS: usize = 512;
/// Longest release note per entry, in characters; shown as plain text, never as markup.
pub const MAX_RELEASE_NOTES_CHARS: usize = 2000;
/// How long an index built without an explicit window stays valid.
pub const DEFAULT_VALIDITY_DAYS: i64 = 90;
/// Longest window between `issued_at` and `expires_at`: how long a replay of a genuine index
/// stays useful to an attacker on a first contact.
pub const MAX_VALIDITY_DAYS: i64 = 366;

/// The signed payload.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PluginIndex {
    /// Always [`PLUGIN_INDEX_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// The publisher's monotonic counter, from 1; never goes backwards.
    pub sequence: u64,
    /// When the publisher signed it.
    pub issued_at: DateTime<Utc>,
    /// After this instant the index is refused, newer or not.
    pub expires_at: DateTime<Utc>,
    /// Every package offered, at most one entry per plugin id and version.
    #[serde(default)]
    pub plugins: Vec<IndexEntry>,
}

/// One downloadable `.rbplugin`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct IndexEntry {
    /// The manifest's `id`.
    pub id: String,
    /// The manifest's English display name.
    pub name: String,
    /// The plugin's semantic version.
    pub version: String,
    /// The `rbrowser:plugin` contract the component was built against.
    pub api_version: String,
    /// The manifest's kinds, as the worlds name them (`feature-plugin`, ...). A kind this build
    /// does not know is listed, not refused: the index is shared by every app version, and the
    /// install decides.
    pub kinds: Vec<String>,
    /// The manifest's description, plain text.
    #[serde(default)]
    pub description: String,
    /// The capabilities the manifest grants, as the manifest names them (`http`, `cdp`, ...):
    /// what the catalogue shows before the install. The install compares them with the package.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// The first rBrowser release the plugin runs on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_app_version: Option<String>,
    /// Absolute `https://` URL, or a path relative to the index's own URL that cannot leave the
    /// index's directory.
    pub url: String,
    /// 64 lowercase hex characters of the SHA-256 of the `.rbplugin` file.
    pub sha256: String,
    /// Byte size of the `.rbplugin` file: a download that grows past it is cut off.
    pub size: u64,
    /// Who signed the package, as its manifest names the key.
    pub publisher: Publisher,
    /// Plain-text notes for this version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_notes: Option<String>,
}

/// The package's signer, as the manifest names it: a claim until the package is downloaded and
/// verified, when the install compares it with the key that actually signed it.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Publisher {
    /// The manifest's `key_id`.
    pub key_id: String,
    /// Lowercase hex SHA-256 of the manifest's public key (`key_fingerprint`).
    pub fingerprint: String,
    /// The manifest's `metadata.author`.
    #[serde(default)]
    pub author: String,
}

/// Why an index was refused; [`code`](Self::code) is what the interface translates.
#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("the plugin index is larger than {MAX_INDEX_BYTES} bytes")]
    TooLarge,
    #[error("the plugin index cannot be read: {0}")]
    Malformed(String),
    #[error("the plugin index is not signed by the repository's key: {0}")]
    Untrusted(String),
    #[error("the plugin index's signature does not verify: {0}")]
    BadSignature(String),
    #[error(
        "the plugin index declares schema version {saw}, this build reads {PLUGIN_INDEX_SCHEMA_VERSION}"
    )]
    SchemaVersion { saw: u64 },
    #[error("the plugin index is stale: {0}")]
    Stale(#[from] StaleError),
    #[error("the plugin index is invalid: {0}")]
    Invalid(String),
}

impl IndexError {
    /// The stable code, an i18n key of `locales/en.json`.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::TooLarge => "plugins.catalog.error.too_large",
            Self::Malformed(_) => "plugins.catalog.error.malformed",
            Self::Untrusted(_) => "plugins.catalog.error.untrusted",
            Self::BadSignature(_) => "plugins.catalog.error.bad_signature",
            Self::SchemaVersion { .. } => "plugins.catalog.error.schema_version",
            Self::Stale(StaleError::Replayed { .. }) => "plugins.catalog.error.replayed",
            Self::Stale(StaleError::Expired { .. }) => "plugins.catalog.error.expired",
            Self::Stale(StaleError::FromTheFuture { .. }) => {
                "plugins.catalog.error.from_the_future"
            }
            Self::Invalid(_) => "plugins.catalog.error.invalid",
        }
    }
}

impl PluginIndex {
    /// The freshness fields, as [`check_freshness`] takes them.
    #[must_use]
    pub fn freshness(&self) -> Freshness {
        Freshness {
            sequence: self.sequence,
            issued_at: self.issued_at,
            expires_at: self.expires_at,
        }
    }
}

impl IndexEntry {
    /// Where to download this package, given the URL the index was fetched from: always
    /// `https://` - a relative entry inherits the index's scheme and host, and an index fetched
    /// over anything else is refused here rather than turning into a plain-HTTP download.
    ///
    /// # Errors
    ///
    /// The URL does not parse or is not https.
    pub fn resolve_url(&self, index_url: &url::Url) -> anyhow::Result<url::Url> {
        let resolved = if self.url.starts_with("https://") {
            url::Url::parse(&self.url)?
        } else {
            index_url.join(&self.url)?
        };
        anyhow::ensure!(
            resolved.scheme() == "https",
            "package URL {resolved} is not https"
        );
        Ok(resolved)
    }
}

/// Verifies a signed index against `signers`.
///
/// `known_sequence` is the highest sequence this installation has accepted *from this
/// repository*; `None` on a first contact and for the very bytes accepted before.
///
/// # Errors
///
/// See [`IndexError`].
pub fn verify(
    bytes: &[u8],
    signers: &IndexSigners,
    known_sequence: Option<u64>,
    now: DateTime<Utc>,
) -> Result<PluginIndex, IndexError> {
    if bytes.len() > MAX_INDEX_BYTES {
        return Err(IndexError::TooLarge);
    }
    let document = SignedDocument::parse(bytes)
        .map_err(|error| IndexError::Malformed(format!("{error:#}")))?;
    let payload: serde_json::Value =
        document
            .verify(PLUGIN_INDEX_DOMAIN, signers)
            .map_err(|error| match error {
                SignatureError::Untrusted { .. } => IndexError::Untrusted(error.to_string()),
                SignatureError::BadSignature { .. } => IndexError::BadSignature(error.to_string()),
                SignatureError::Malformed(other) => IndexError::Malformed(format!("{other:#}")),
            })?;
    let saw = payload
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| IndexError::Malformed("schema_version is missing".to_owned()))?;
    if saw != u64::from(PLUGIN_INDEX_SCHEMA_VERSION) {
        return Err(IndexError::SchemaVersion { saw });
    }
    let index: PluginIndex = serde_json::from_value(payload)
        .map_err(|error| IndexError::Malformed(error.to_string()))?;
    check_freshness(index.freshness(), known_sequence, now)?;
    index.validate()?;
    Ok(index)
}

/// Signs `index` under `key_id` and returns the document as it is published. Refuses an index
/// [`verify`] would refuse for its content or size, so a broken index never leaves the
/// publisher.
///
/// # Errors
///
/// The index is invalid or too large.
pub fn sign(key_id: &str, key: &SigningKey, index: &PluginIndex) -> anyhow::Result<Vec<u8>> {
    index.validate()?;
    let document = sign_document(PLUGIN_INDEX_DOMAIN, key_id, key, index)?;
    let bytes = serde_json::to_vec_pretty(&document)?;
    anyhow::ensure!(
        bytes.len() <= MAX_INDEX_BYTES,
        "the signed index is {} bytes, more than the {MAX_INDEX_BYTES} a reader accepts",
        bytes.len()
    );
    Ok(bytes)
}

#[cfg(test)]
#[path = "index_tests.rs"]
mod tests;
