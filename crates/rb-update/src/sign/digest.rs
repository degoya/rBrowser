//! The framing every signed document of this crate is hashed with: **every field is
//! length-prefixed** with its big-endian `u64` length, so a member boundary cannot move without
//! changing the digest. Frozen once a manifest is published under it; `digest_tests.rs` pins it
//! with a byte literal.
//!
//! Copied from rCommander's `crates/rc-update/src/sign/digest.rs` (RB-090-02), which took it
//! from rDownloader's `rd-sign`.

use sha2::{Digest, Sha256};

/// Accumulates the length-prefixed payload that gets signed, in call order.
#[derive(Clone, Debug, Default)]
pub struct DigestBuilder {
    hasher: Sha256,
}

impl DigestBuilder {
    /// Starts an empty payload.
    #[must_use]
    pub fn new() -> Self {
        Self {
            hasher: Sha256::new(),
        }
    }

    /// Appends one length-prefixed field.
    pub fn field(&mut self, bytes: &[u8]) -> &mut Self {
        self.hasher.update((bytes.len() as u64).to_be_bytes());
        self.hasher.update(bytes);
        self
    }

    /// Finishes the payload.
    #[must_use]
    pub fn finish(self) -> [u8; 32] {
        self.hasher.finalize().into()
    }
}

/// Lowercase hex of `bytes` (`rb_util::hex`), under the path the crate always used.
pub use rb_util::hex;

/// 64 lowercase hexadecimal digits: the form of every SHA-256 a manifest or a journal names.
#[must_use]
pub fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Hex SHA-256 of `bytes`, the form every digest in a manifest takes.
#[must_use]
pub fn hex_sha256(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

#[cfg(test)]
#[path = "digest_tests.rs"]
mod tests;
