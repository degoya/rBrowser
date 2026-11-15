//! The plugin signing keys compiled into the binary, and how they rotate.
//!
//! Copied and adapted from rCommander's `crates/rc-plugin-host/src/sign/roots.rs` (rDownloader's
//! `rd-sign`): a table rather than one constant, so a key can be rotated with an overlap window
//! (`not_after`, Unix seconds). A package a root signs is trusted without a question - the
//! bundled first-party plugins are.
//!
//! **Key setup** (`docs/plugins.md`, "Signing keys"): the release key `rbrowser-plugin-v1` signs
//! the first-party plugins. Its private half never enters the repository; the owner makes it once
//! with `rb-plugin-pack keygen --out <outside the repository>` and writes its public half below.
//! Until then the entry is empty - "not configured in this build" - so no package verifies as
//! first-party, and `scripts/build-plugins.sh` refuses to sign without a key. No development key is
//! compiled in: a development build that wants unsigned plugins turns on development mode, which
//! marks them as such everywhere.

use std::time::{SystemTime, UNIX_EPOCH};

use super::trust::TrustStore;

/// One compiled-in public key.
#[derive(Clone, Copy, Debug)]
pub struct EmbeddedKey {
    /// The `key_id` a manifest signed with it names.
    pub key_id: &'static str,
    /// Base64 Ed25519 public key; empty means "not configured in this build".
    pub public_key: &'static str,
    /// Unix seconds after which the key is no longer accepted, when it is rotated out.
    pub not_after: Option<u64>,
}

/// The release key the first-party plugins are signed with.
pub const RELEASE_KEY_ID: &str = "rbrowser-plugin-v1";

/// Every plugin root this build ships.
pub const EMBEDDED_KEYS: &[EmbeddedKey] = &[EmbeddedKey {
    key_id: RELEASE_KEY_ID,
    // The owner's release key, made 2026-11-10 with `rb-plugin-pack keygen`; the private half is
    // ~/.config/rbrowser/rbrowser-plugin.key, never in a repository (docs/plugins.md, "Signing
    // keys").
    public_key: "NUD3ef9GLk8/CfUz6W8+hZrflwlJO3ku9MmC8+7lk9g=",
    not_after: None,
}];

/// The configured, unexpired roots at the current instant.
#[must_use]
pub fn root_keys() -> Vec<&'static EmbeddedKey> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    root_keys_at(EMBEDDED_KEYS, now)
}

/// The configured roots of `table` still valid at `now` (Unix seconds); an entry without a key is
/// skipped rather than reported.
#[must_use]
pub fn root_keys_at(table: &'static [EmbeddedKey], now: u64) -> Vec<&'static EmbeddedKey> {
    table
        .iter()
        .filter(|entry| !entry.public_key.is_empty())
        .filter(|entry| entry.not_after.is_none_or(|end| now <= end))
        .collect()
}

impl TrustStore {
    /// A store holding every configured root.
    ///
    /// # Errors
    ///
    /// A root does not decode: the build is broken, and nothing is trusted rather than less.
    pub fn with_roots() -> anyhow::Result<Self> {
        let store = Self::new();
        for entry in root_keys() {
            store.trust_base64(entry.key_id.to_owned(), entry.public_key)?;
        }
        Ok(store)
    }
}

#[cfg(test)]
#[path = "roots_tests.rs"]
mod tests;
