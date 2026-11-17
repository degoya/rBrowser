//! The review of a downloaded package before it is installed (RB-091-01, security review H1 and
//! M1): the package verified as a `.rbplugin` file is, held to what its index entry describes,
//! and the state of its publisher key - trusted here or not, and whether it is the key the
//! installed version was signed with. Nothing is trusted, pinned or written here: the interface
//! shows the package's full grants and its key, and only [`super::install`] with the person's
//! [`super::Consent`] goes further.
//!
//! Adapted from rDownloader's `repository_prepare.rs` (RD-140-01).

use std::collections::BTreeSet;

use serde::Serialize;

use super::{CatalogStore, RepositoryError};
use crate::index::IndexEntry;
use crate::manifest::PluginManifest;
use crate::package::VerifiedPackage;
use crate::sign::{decode_public_key, hex_sha256, key_fingerprint};
use crate::verifier::{PluginVerifier, VerifyError};

/// The publisher key of a reviewed package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyReview {
    pub key_id: String,
    pub fingerprint: String,
    /// Whether this installation trusts the key already (a root or `plugins.trustedKeys`).
    pub trusted: bool,
    /// The key of the version the catalogue installed before, when there is one.
    pub previous_fingerprint: Option<String>,
}

impl KeyReview {
    /// Whether the update is signed by another key than the installed version.
    #[must_use]
    pub fn changed(&self) -> bool {
        self.previous_fingerprint
            .as_deref()
            .is_some_and(|previous| previous != self.fingerprint)
    }

    /// Whether the person has to confirm the key in a step of its own: a key not trusted yet,
    /// or another key than the installed version's.
    #[must_use]
    pub fn needs_confirmation(&self) -> bool {
        !self.trusted || self.changed()
    }
}

/// A downloaded package that verified and is what its index entry describes.
pub struct Prepared {
    package: VerifiedPackage,
    key: KeyReview,
    public_key: String,
}

impl Prepared {
    /// The package's manifest: its grants are what the install dialog shows.
    #[must_use]
    pub fn manifest(&self) -> &PluginManifest {
        &self.package.manifest
    }

    #[must_use]
    pub fn key(&self) -> &KeyReview {
        &self.key
    }

    /// The package's base64 public key, pinned when the person confirms an untrusted one.
    #[must_use]
    pub fn public_key(&self) -> &str {
        &self.public_key
    }

    /// The capability ids the package's manifest grants.
    #[must_use]
    pub fn capability_ids(&self) -> BTreeSet<&'static str> {
        self.package
            .manifest
            .grants
            .granted
            .iter()
            .map(|capability| capability.id())
            .collect()
    }
}

/// The size and the SHA-256 the index names.
pub(super) fn matches_entry(bytes: &[u8], entry: &IndexEntry) -> Result<(), RepositoryError> {
    let got = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if got != entry.size {
        return Err(RepositoryError::SizeMismatch {
            expected: entry.size,
            got,
        });
    }
    if hex_sha256(bytes) != entry.sha256 {
        return Err(RepositoryError::ChecksumMismatch);
    }
    Ok(())
}

/// Verifies `bytes` as the package `entry` describes, under `verifier`'s keys; a key `verifier`
/// does not trust is checked for consistency by a verifier of its own and reported, never
/// trusted. `store` names the key of an installed version.
///
/// # Errors
///
/// The bytes are not the indexed file, the package does not verify or is unsigned, or it is not
/// what the index describes.
pub fn prepare(
    bytes: &[u8],
    entry: &IndexEntry,
    verifier: &PluginVerifier,
    store: &CatalogStore,
) -> Result<Prepared, RepositoryError> {
    matches_entry(bytes, entry)?;
    let (package, trusted) = match verifier.verify_bytes(bytes) {
        Ok(package) => (package, true),
        Err(VerifyError::UntrustedKey {
            key_id, public_key, ..
        }) => {
            // Only to read the package whose signature holds under its own key; this verifier is
            // dropped here, and `verifier`, whose store the app shares, is never changed.
            let scoped = PluginVerifier::new(false);
            scoped
                .trust_key_base64(key_id, &public_key)
                .map_err(|_| RepositoryError::NotAsDescribed("public_key".to_owned()))?;
            (scoped.verify_bytes(bytes).map_err(refused)?, false)
        }
        Err(error) => return Err(refused(error)),
    };
    if package.signature.is_none() {
        return Err(RepositoryError::Unsigned);
    }
    let fingerprint = as_described(&package, entry)?;
    let previous_fingerprint = store
        .installs()
        .get(&package.manifest.id)
        .and_then(|record| record.fingerprint.clone());
    Ok(Prepared {
        key: KeyReview {
            key_id: package.manifest.key_id.clone(),
            fingerprint,
            trusted,
            previous_fingerprint,
        },
        public_key: package.manifest.public_key.clone(),
        package,
    })
}

fn refused(error: VerifyError) -> RepositoryError {
    RepositoryError::Package {
        code: error.code(),
        reason: format!("{error:#}"),
    }
}

/// The package is the one the index described - id, version, signer, capabilities - and its key's
/// fingerprint.
fn as_described(package: &VerifiedPackage, entry: &IndexEntry) -> Result<String, RepositoryError> {
    let manifest = &package.manifest;
    let not = |what: &str| RepositoryError::NotAsDescribed(what.to_owned());
    if manifest.id != entry.id {
        return Err(not("id"));
    }
    if manifest.version.to_string() != entry.version {
        return Err(not("version"));
    }
    let fingerprint = decode_public_key(&manifest.public_key)
        .map(|key| key_fingerprint(&key))
        .map_err(|_| not("public_key"))?;
    if manifest.key_id != entry.publisher.key_id || fingerprint != entry.publisher.fingerprint {
        return Err(not("publisher"));
    }
    let granted: BTreeSet<&str> = manifest
        .grants
        .granted
        .iter()
        .map(|capability| capability.id())
        .collect();
    let listed: BTreeSet<&str> = entry.capabilities.iter().map(String::as_str).collect();
    if granted != listed {
        return Err(not("capabilities"));
    }
    Ok(fingerprint)
}
