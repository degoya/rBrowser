//! Index fixtures for the tests of the index, the merge and the repositories: a repository key,
//! a fixed clock and entries that pass every rule, to be changed one field at a time.

use chrono::{DateTime, Duration, Utc};
use ed25519_dalek::SigningKey;

use super::{
    IndexEntry, IndexSigners, PLUGIN_INDEX_DOMAIN, PLUGIN_INDEX_SCHEMA_VERSION, PluginIndex,
    Publisher, sign, sign_document,
};
use crate::sign::TrustStore;

/// The key id of the fixture repository.
pub(crate) const REPOSITORY_KEY_ID: &str = "test-repository";
/// A well-formed SHA-256 that matches no package.
pub(crate) const SHA256: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const FINGERPRINT: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

/// The instant every fixture is verified at.
pub(crate) fn now() -> DateTime<Utc> {
    DateTime::from_timestamp(1_790_000_000, 0).expect("timestamp")
}

/// The repository's signing key.
pub(crate) fn repository_key() -> SigningKey {
    SigningKey::from_bytes(&[9; 32])
}

/// The repository's key as the official catalogue holds its roots.
pub(crate) fn roots() -> IndexSigners {
    let store = TrustStore::new();
    store
        .trust(
            REPOSITORY_KEY_ID.to_owned(),
            repository_key().verifying_key(),
        )
        .expect("trust");
    IndexSigners::Roots(store)
}

/// The repository's key as a person pins it.
pub(crate) fn pinned() -> IndexSigners {
    IndexSigners::Pinned(repository_key().verifying_key())
}

/// An entry of plugin `id` at `version` that passes every rule.
pub(crate) fn entry(id: &str, version: &str) -> IndexEntry {
    IndexEntry {
        id: id.to_owned(),
        name: "Fixture".to_owned(),
        version: version.to_owned(),
        api_version: rb_plugin_api::API_VERSION.to_owned(),
        kinds: vec!["feature-plugin".to_owned()],
        description: "A plugin of the index tests.".to_owned(),
        capabilities: vec!["views".to_owned(), "http".to_owned()],
        min_app_version: None,
        category: None,
        url: format!("{id}-{version}.rbplugin"),
        sha256: SHA256.to_owned(),
        size: 4096,
        publisher: Publisher {
            key_id: "fixture-v1".to_owned(),
            fingerprint: FINGERPRINT.to_owned(),
            author: "Fixture Author".to_owned(),
        },
        release_notes: Some("Fixes the counter.\nNo new capabilities.".to_owned()),
    }
}

/// An index at `sequence` listing `plugins`, issued an hour ago and valid for 30 days.
pub(crate) fn index_of(sequence: u64, plugins: Vec<IndexEntry>) -> PluginIndex {
    PluginIndex {
        schema_version: PLUGIN_INDEX_SCHEMA_VERSION,
        sequence,
        issued_at: now() - Duration::hours(1),
        expires_at: now() + Duration::days(30),
        plugins,
    }
}

/// The default index: sequence 5, one plugin.
pub(crate) fn index() -> PluginIndex {
    index_of(5, vec![entry("fixture", "1.2.3")])
}

/// `index` signed by the repository key.
pub(crate) fn signed(index: &PluginIndex) -> Vec<u8> {
    sign(REPOSITORY_KEY_ID, &repository_key(), index).expect("sign")
}

/// A payload [`sign`] would refuse, signed by the repository key, so a refusal can be tested
/// end to end.
pub(crate) fn signed_raw(payload: &serde_json::Value) -> Vec<u8> {
    let document = sign_document(
        PLUGIN_INDEX_DOMAIN,
        REPOSITORY_KEY_ID,
        &repository_key(),
        payload,
    )
    .expect("sign");
    serde_json::to_vec(&document).expect("encode")
}
