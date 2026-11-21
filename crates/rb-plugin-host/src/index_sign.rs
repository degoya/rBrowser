//! The signed document a plugin index travels in, and its freshness rules (RB-091-01).
//!
//! Adapted from rb-update's `sign/envelope.rs` and `sign/replay.rs` (themselves rDownloader's
//! `rd-sign`), on the host's own digest and detached signatures (`crate::sign`), so the plugin
//! host does not link the self-update crate.
//!
//! **The payload is signed as the bytes it arrived as**, kept verbatim in a `RawValue` and never
//! re-serialised: canonicalising JSON before hashing gives a producer and a verifier room to
//! disagree about what "the same document" is. **Every signature is bound to a domain string**
//! (`rbrowser.plugin-index.v1`) hashed before the payload, so a signature over an index cannot be
//! lifted onto another kind of document under the same key - the official index is signed with
//! the plugin release key, which also signs the first-party packages.
//!
//! **Freshness**: a signature says who wrote a document, never when. The sequence never goes
//! backwards, `expires_at` bounds how long a replay stays useful on a first contact, and an
//! `issued_at` further ahead than clock skew explains is refused.

use anyhow::{Context, Result};
use ed25519_dalek::{SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::value::RawValue;
use time::{OffsetDateTime, SignedDuration};

use crate::sign::{DigestBuilder, TrustStore, sign_detached, verify_detached};

/// The only signature algorithm this format admits.
pub const ALGORITHM_ED25519: &str = "ed25519";
/// How far ahead of local time an `issued_at` may sit before it stops being clock skew.
pub const MAX_CLOCK_SKEW_MINUTES: i64 = 60;
/// Most signatures one document may carry; each is one verification.
const MAX_SIGNATURES: usize = 8;

/// One detached signature over a document's payload.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentSignature {
    /// Which key is claimed - never the key itself, which would verify against itself.
    pub key_id: String,
    /// Always [`ALGORITHM_ED25519`]; present so a future algorithm is a refused value.
    pub algorithm: String,
    /// Base64 of the raw 64-byte signature.
    pub signature: String,
}

/// A payload plus the signatures over it.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SignedDocument {
    payload: Box<RawValue>,
    signatures: Vec<DocumentSignature>,
}

/// The keys an index may be signed with.
#[derive(Clone, Debug)]
pub enum IndexSigners {
    /// The official catalogue: the plugin roots compiled into the app, by key id.
    Roots(TrustStore),
    /// A repository a person added: exactly this key, whatever key id the document names - a
    /// repository is known by its key, not by a name it chooses.
    Pinned(VerifyingKey),
}

impl IndexSigners {
    /// The official catalogue's signers: the compiled-in plugin release key(s).
    ///
    /// # Errors
    ///
    /// A root does not decode.
    pub fn official() -> Result<Self> {
        Ok(Self::Roots(TrustStore::with_roots()?))
    }

    fn key(&self, key_id: &str) -> Result<Option<VerifyingKey>> {
        match self {
            Self::Roots(store) => store.key(key_id),
            Self::Pinned(key) => Ok(Some(*key)),
        }
    }
}

/// Why a signed document was not accepted.
#[derive(Debug, thiserror::Error)]
pub enum SignatureError {
    /// No signature named a key this repository is trusted with.
    #[error("no signature from a trusted key (saw: {key_ids})")]
    Untrusted { key_ids: String },
    /// A signature named a trusted key but did not verify: tampering or corruption.
    #[error("the signature from {key_id} does not verify")]
    BadSignature { key_id: String },
    /// The bytes are no signed document.
    #[error(transparent)]
    Malformed(#[from] anyhow::Error),
}

impl SignedDocument {
    /// Parses a document without verifying anything; [`Self::verify`] is the only way to its
    /// payload.
    ///
    /// # Errors
    ///
    /// The bytes are no signed document.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let document: Self = serde_json::from_slice(bytes).context("parse signed document")?;
        anyhow::ensure!(
            document.signatures.len() <= MAX_SIGNATURES,
            "more than {MAX_SIGNATURES} signatures"
        );
        Ok(document)
    }

    /// The digest this document's signatures are taken over: the domain, then the payload's
    /// bytes, each length-prefixed.
    #[must_use]
    pub fn digest(&self, domain: &str) -> [u8; 32] {
        let mut builder = DigestBuilder::new();
        builder.field(domain.as_bytes());
        builder.field(self.payload.get().as_bytes());
        builder.finish()
    }

    /// Verifies against `signers` and returns the parsed payload. One valid signature from a
    /// trusted key is enough.
    ///
    /// # Errors
    ///
    /// See [`SignatureError`].
    pub fn verify<T: DeserializeOwned>(
        &self,
        domain: &str,
        signers: &IndexSigners,
    ) -> Result<T, SignatureError> {
        let digest = self.digest(domain);
        let mut named_a_trusted_key = None;
        for entry in &self.signatures {
            if entry.algorithm != ALGORITHM_ED25519 {
                continue;
            }
            let Some(key) = signers.key(&entry.key_id)? else {
                continue;
            };
            named_a_trusted_key = Some(entry.key_id.clone());
            if verify_detached(&key, &digest, &entry.signature).is_ok() {
                return serde_json::from_str(self.payload.get())
                    .context("parse signed document payload")
                    .map_err(SignatureError::Malformed);
            }
        }
        match named_a_trusted_key {
            Some(key_id) => Err(SignatureError::BadSignature { key_id }),
            None => Err(SignatureError::Untrusted {
                key_ids: self
                    .signatures
                    .iter()
                    .map(|entry| entry.key_id.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            }),
        }
    }
}

/// Wraps `payload` in a document signed by `key` under `key_id` for `domain`.
///
/// # Errors
///
/// The payload does not serialise to JSON.
pub fn sign_document<T: Serialize>(
    domain: &str,
    key_id: &str,
    key: &SigningKey,
    payload: &T,
) -> Result<SignedDocument> {
    let serialised = serde_json::to_string(payload).context("serialise document payload")?;
    let raw = RawValue::from_string(serialised).context("payload is not a JSON value")?;
    let mut document = SignedDocument {
        payload: raw,
        signatures: Vec::new(),
    };
    let digest = document.digest(domain);
    document.signatures.push(DocumentSignature {
        key_id: key_id.to_owned(),
        algorithm: ALGORITHM_ED25519.to_owned(),
        signature: sign_detached(key, &digest),
    });
    Ok(document)
}

/// The freshness fields of a signed index.
#[derive(Clone, Copy, Debug)]
pub struct Freshness {
    /// The publisher's monotonic counter.
    pub sequence: u64,
    /// When the publisher says it signed this.
    pub issued_at: OffsetDateTime,
    /// After this instant the document is stale even if nothing newer has been seen.
    pub expires_at: OffsetDateTime,
}

/// Why a document was refused as stale.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum StaleError {
    #[error("index sequence {saw} is older than the {known} already accepted")]
    Replayed { saw: u64, known: u64 },
    #[error("the index expired at {}", crate::utc::display(.expires_at))]
    Expired { expires_at: OffsetDateTime },
    #[error(
        "the index is issued {}, further ahead than clock skew explains",
        crate::utc::display(.issued_at)
    )]
    FromTheFuture { issued_at: OffsetDateTime },
}

/// Checks `candidate` against the highest sequence already accepted from the same repository;
/// `known` is `None` on a first contact, where only the expiry and the skew bound apply.
///
/// The same sequence is a replay; the caller passes `None` when the bytes are exactly the ones
/// already accepted (an unchanged index fetched again), and checks its floor itself.
///
/// # Errors
///
/// See [`StaleError`].
pub fn check_freshness(
    candidate: Freshness,
    known: Option<u64>,
    now: OffsetDateTime,
) -> Result<(), StaleError> {
    if let Some(known) = known
        && candidate.sequence <= known
    {
        return Err(StaleError::Replayed {
            saw: candidate.sequence,
            known,
        });
    }
    if now > candidate.expires_at {
        return Err(StaleError::Expired {
            expires_at: candidate.expires_at,
        });
    }
    if candidate.issued_at > now + SignedDuration::minutes(MAX_CLOCK_SKEW_MINUTES) {
        return Err(StaleError::FromTheFuture {
            issued_at: candidate.issued_at,
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "index_sign_tests.rs"]
mod tests;
