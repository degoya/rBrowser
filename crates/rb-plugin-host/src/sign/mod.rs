//! Signatures, digests and trust for plugin packages: the length-prefixed digest, detached
//! Ed25519 signatures over it, the keys this installation trusts and the roots compiled in.
//!
//! Copied from rCommander's `crates/rc-plugin-host/src/sign.rs` and `sign/` (themselves from
//! rDownloader's `rd-sign`), unchanged in substance: a module of the host, since rBrowser signs
//! nothing but plugin packages. The framing of the digest is the same as rCommander's.

use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, Signer as _, SigningKey, Verifier as _, VerifyingKey};

mod digest;
mod roots;
mod trust;

pub use digest::{DigestBuilder, hex_sha256};
pub use roots::{EMBEDDED_KEYS, EmbeddedKey, RELEASE_KEY_ID, root_keys, root_keys_at};
pub use trust::{TrustStore, decode_public_key, key_fingerprint};

/// Verifies one base64 Ed25519 signature over `digest`.
///
/// # Errors
///
/// The text is no base64 signature, or it does not verify.
pub fn verify_detached(key: &VerifyingKey, digest: &[u8; 32], encoded: &str) -> Result<()> {
    let raw = STANDARD
        .decode(encoded.trim())
        .context("decode signature")?;
    let signature = Signature::from_slice(&raw).context("invalid Ed25519 signature")?;
    key.verify(digest, &signature)
        .context("signature verification failed")
}

/// [`verify_detached`] over a signature as an archive member holds it.
///
/// # Errors
///
/// The bytes are no UTF-8 base64 signature, or it does not verify.
pub fn verify_detached_bytes(key: &VerifyingKey, digest: &[u8; 32], encoded: &[u8]) -> Result<()> {
    let text = std::str::from_utf8(encoded).context("signature is not UTF-8")?;
    verify_detached(key, digest, text)
}

/// Signs `digest`; the base64 form `signature.ed25519` holds.
#[must_use]
pub fn sign_detached(key: &SigningKey, digest: &[u8; 32]) -> String {
    STANDARD.encode(key.sign(digest).to_bytes())
}

#[cfg(test)]
#[path = "sign_tests.rs"]
mod tests;
