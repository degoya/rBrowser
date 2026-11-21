//! A served repository for the catalogue's tests: a real signed package, its index signed by the
//! fixture repository key, both in a [`MemoryFetcher`].

use std::path::PathBuf;

use ed25519_dalek::SigningKey;

use super::{CatalogStore, MemoryFetcher, Repository};
use crate::PluginVerifier;
use crate::index::fixtures::{index_of, now, repository_key, roots, signed};
use crate::index::{IndexEntry, describe_package, package_file_name};
use crate::test_support::{KEY_ID, ManifestBuilder, signing_key};
use crate::testbed::Bed;

/// The official fixture repository's index address.
pub(crate) const OFFICIAL_URL: &str = "https://official.test/rbrowser-plugin-index.json";
/// A person's fixture repository's index address.
pub(crate) const MINE_URL: &str = "https://mine.test/repo/rbrowser-plugin-index.json";

/// The package author's key.
pub(crate) fn author_key() -> SigningKey {
    signing_key(1)
}

/// The fixture package `sample` 1.0.0 with the capability `views`, and its index entry with a
/// relative URL.
pub(crate) fn sample_package() -> (Vec<u8>, IndexEntry) {
    let file = ManifestBuilder::default()
        .capability("views = true")
        .package(&Bed::feature().build(), &author_key(), true, &[]);
    let package = trusting_author().verify_bytes(&file).expect("verifies");
    let entry = describe_package(&file, &package, &package_file_name("sample", "1.0.0"), None)
        .expect("describe");
    (file, entry)
}

/// A verifier trusting the package author's key.
pub(crate) fn trusting_author() -> PluginVerifier {
    let verifier = PluginVerifier::new(false);
    verifier
        .trust_key(KEY_ID.to_owned(), author_key().verifying_key())
        .expect("trust");
    verifier
}

/// The official fixture repository, signed with the fixture key as its root.
pub(crate) fn official() -> Repository {
    Repository::official_with(OFFICIAL_URL, roots()).expect("official")
}

/// A person's fixture repository, pinned to the same fixture key.
pub(crate) fn mine() -> Repository {
    let public = crate::public_key_base64(&repository_key());
    Repository::user("Mine", MINE_URL, &public).expect("user repository")
}

/// Serves `entries` as the index at `url`, at `sequence`.
pub(crate) fn serve_index(
    fetcher: &MemoryFetcher,
    url: &str,
    sequence: u64,
    entries: Vec<IndexEntry>,
) {
    fetcher.serve(url, signed(&index_of(sequence, entries)));
}

/// A store and a user plugin folder in a fresh temporary folder.
pub(crate) fn folders() -> (tempfile::TempDir, CatalogStore, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = CatalogStore::new(dir.path().join("plugin-catalog"));
    let user = dir.path().join("plugins");
    (dir, store, user)
}

/// The fixture clock.
pub(crate) fn at() -> time::OffsetDateTime {
    now()
}
