//! Installing from the catalogue (RB-091-01): a wrong SHA-256 or size installs nothing, the
//! package is held to its entry and to the person's consent, a key is never trusted as a side
//! effect (security review H1), nothing the catalogue did not write is replaced and an update must
//! be newer (L2), and only the catalogue's own installs can be removed.

use super::super::fixtures::{
    OFFICIAL_URL, at, author_key, folders, official, sample_package, serve_index, trusting_author,
};
use super::super::prepare::prepare;
use super::super::{Catalog, CatalogStore, InstallRecord, MemoryFetcher, RepositoryError};
use super::{Consent, download, install, remove};
use crate::PluginVerifier;
use crate::index::{IndexEntry, describe_package};
use crate::sign::key_fingerprint;
use crate::test_support::ManifestBuilder;
use crate::testbed::Bed;

const PACKAGE_URL: &str = "https://official.test/sample-1.0.0.rbplugin";

fn views() -> Consent {
    Consent {
        capabilities: vec!["views".to_owned()],
        key_fingerprint: None,
    }
}

fn author_fingerprint() -> String {
    key_fingerprint(&author_key().verifying_key())
}

fn index_url() -> url::Url {
    url::Url::parse(OFFICIAL_URL).expect("url")
}

/// Reviews and installs `file` under `verifier` with `consent`.
fn review_and_install(
    file: &[u8],
    entry: &IndexEntry,
    verifier: &PluginVerifier,
    consent: &Consent,
    user: &std::path::Path,
    store: &CatalogStore,
) -> Result<super::Installed, RepositoryError> {
    let prepared = prepare(file, entry, verifier, store)?;
    install(&prepared, file, "official", entry, consent, user, store)
}

#[test]
fn an_offered_package_downloads_verifies_and_installs_into_the_user_folder() {
    let (_dir, store, user) = folders();
    let (file, entry) = sample_package();
    let fetcher = MemoryFetcher::new();
    fetcher.serve(PACKAGE_URL, file.clone());
    serve_index(&fetcher, OFFICIAL_URL, 2, vec![entry]);
    let mut catalog = Catalog::new(store.clone(), vec![official()]);
    catalog.refresh(&fetcher, at());
    let (url, offered) = catalog
        .offer("official", "sample", "1.0.0", at())
        .expect("offered");
    let bytes = download(&fetcher, &url, &offered).expect("download");
    let installed = review_and_install(
        &bytes,
        &offered,
        &trusting_author(),
        &views(),
        &user,
        &store,
    )
    .expect("install");
    assert_eq!(installed.path, user.join("sample.rbplugin"));
    assert_eq!(std::fs::read(&installed.path).expect("written"), file);
    assert!(installed.pin.is_none());
    let record = &store.installs()["sample"];
    assert_eq!(record.repository, "official");
    assert_eq!(
        record.fingerprint.as_deref(),
        Some(author_fingerprint().as_str())
    );
}

#[test]
fn a_package_with_a_wrong_sha256_is_not_installed() {
    let (_dir, store, user) = folders();
    let (file, mut entry) = sample_package();
    entry.sha256 = "ab".repeat(32);
    let fetcher = MemoryFetcher::new();
    fetcher.serve(PACKAGE_URL, file.clone());
    assert!(matches!(
        download(&fetcher, &index_url(), &entry),
        Err(RepositoryError::ChecksumMismatch)
    ));
    let refused = review_and_install(&file, &entry, &trusting_author(), &views(), &user, &store);
    assert_eq!(
        refused.err().map(|error| error.code()),
        Some("plugins.catalog.error.checksum")
    );
    assert!(!user.join("sample.rbplugin").exists());
    assert!(store.installs().is_empty());
}

#[test]
fn a_download_stays_within_the_size_the_index_names() {
    let (file, mut entry) = sample_package();
    let fetcher = MemoryFetcher::new();
    fetcher.serve(PACKAGE_URL, file.clone());
    let size = u64::try_from(file.len()).expect("size");
    entry.size = size - 1;
    assert!(matches!(
        download(&fetcher, &index_url(), &entry),
        Err(RepositoryError::TooLarge { .. })
    ));
    entry.size = size + 1;
    assert!(matches!(
        download(&fetcher, &index_url(), &entry),
        Err(RepositoryError::SizeMismatch { .. })
    ));
}

#[test]
fn other_capabilities_than_the_confirmed_ones_install_nothing() {
    let (_dir, store, user) = folders();
    let (file, entry) = sample_package();
    let more = Consent {
        capabilities: vec!["views".to_owned(), "http".to_owned()],
        key_fingerprint: None,
    };
    for consent in [Consent::default(), more] {
        let refused =
            review_and_install(&file, &entry, &trusting_author(), &consent, &user, &store);
        assert!(matches!(refused, Err(RepositoryError::ConsentMismatch)));
    }
    assert!(!user.join("sample.rbplugin").exists());
}

#[test]
fn an_index_that_hides_a_capability_is_not_believed() {
    let (_dir, store, _user) = folders();
    let (file, mut entry) = sample_package();
    entry.capabilities.clear();
    let refused = prepare(&file, &entry, &trusting_author(), &store);
    assert!(matches!(refused, Err(RepositoryError::NotAsDescribed(_))));
}

#[test]
fn an_unknown_key_is_reported_by_the_review_and_trusted_only_in_its_own_confirmation() {
    let (_dir, store, user) = folders();
    let (file, entry) = sample_package();
    let stranger = PluginVerifier::new(false);
    let prepared = prepare(&file, &entry, &stranger, &store).expect("reviewed");
    assert!(!prepared.key().trusted);
    assert_eq!(prepared.key().fingerprint, author_fingerprint());
    // The capabilities alone never trust the key (H1).
    let refused = install(
        &prepared,
        &file,
        "official",
        &entry,
        &views(),
        &user,
        &store,
    );
    assert!(matches!(refused, Err(RepositoryError::KeyUntrusted { .. })));
    let wrong = Consent {
        key_fingerprint: Some("00".repeat(32)),
        ..views()
    };
    let refused = install(&prepared, &file, "official", &entry, &wrong, &user, &store);
    assert!(matches!(refused, Err(RepositoryError::KeyUntrusted { .. })));
    assert!(!user.join("sample.rbplugin").exists());
    let confirmed = Consent {
        key_fingerprint: Some(author_fingerprint()),
        ..views()
    };
    let installed = install(
        &prepared, &file, "official", &entry, &confirmed, &user, &store,
    )
    .expect("install");
    assert_eq!(
        installed.pin.expect("a key to pin").fingerprint,
        author_fingerprint()
    );
    assert!(
        stranger.verify_bytes(&file).is_err(),
        "the shared verifier was not changed"
    );
}

#[test]
fn an_update_signed_by_another_key_needs_its_own_confirmation() {
    let (_dir, store, user) = folders();
    let (file, entry) = sample_package();
    store
        .record_install(
            "sample",
            Some(InstallRecord {
                repository: "official".to_owned(),
                version: "0.9.0".to_owned(),
                sha256: "00".repeat(32),
                fingerprint: Some("cd".repeat(32)),
            }),
        )
        .expect("record");
    let prepared = prepare(&file, &entry, &trusting_author(), &store).expect("reviewed");
    assert!(prepared.key().trusted && prepared.key().changed());
    let refused = install(
        &prepared,
        &file,
        "official",
        &entry,
        &views(),
        &user,
        &store,
    );
    assert!(matches!(refused, Err(RepositoryError::KeyChanged { .. })));
    let confirmed = Consent {
        key_fingerprint: Some(author_fingerprint()),
        ..views()
    };
    let installed = install(
        &prepared, &file, "official", &entry, &confirmed, &user, &store,
    )
    .expect("install");
    assert!(installed.pin.is_none(), "a trusted key is not pinned again");
}

#[test]
fn a_foreign_file_is_never_replaced_and_an_update_must_be_newer() {
    let (_dir, store, user) = folders();
    let (file, entry) = sample_package();
    std::fs::create_dir_all(&user).expect("folder");
    std::fs::write(user.join("sample.rbplugin"), b"copied in by hand").expect("write");
    let refused = review_and_install(&file, &entry, &trusting_author(), &views(), &user, &store);
    assert!(matches!(refused, Err(RepositoryError::WouldOverwrite(_))));
    assert_eq!(
        std::fs::read(user.join("sample.rbplugin")).expect("kept"),
        b"copied in by hand"
    );
    std::fs::remove_file(user.join("sample.rbplugin")).expect("remove");
    review_and_install(&file, &entry, &trusting_author(), &views(), &user, &store)
        .expect("first install");
    let again = review_and_install(&file, &entry, &trusting_author(), &views(), &user, &store);
    assert!(matches!(again, Err(RepositoryError::NotNewer { .. })));
}

#[test]
fn an_unsigned_package_is_refused_even_in_development_mode() {
    let (_dir, store, _user) = folders();
    let file = ManifestBuilder::default()
        .capability("views = true")
        .package(&Bed::feature().build(), &author_key(), false, &[]);
    let development = PluginVerifier::new(true);
    let package = development.verify_bytes(&file).expect("dev accepts it");
    let entry = describe_package(&file, &package, "sample-1.0.0.rbplugin", None).expect("entry");
    let refused = prepare(&file, &entry, &development, &store);
    assert!(matches!(refused, Err(RepositoryError::Unsigned)));
}

#[test]
fn only_the_catalogues_own_installs_are_removed() {
    let (_dir, store, user) = folders();
    std::fs::create_dir_all(&user).expect("folder");
    std::fs::write(user.join("hand.rbplugin"), b"copied in by hand").expect("write");
    assert!(matches!(
        remove("hand", &user, &store),
        Err(RepositoryError::NotInstalledHere)
    ));
    assert!(user.join("hand.rbplugin").exists());
    let (file, entry) = sample_package();
    review_and_install(&file, &entry, &trusting_author(), &views(), &user, &store)
        .expect("install");
    remove("sample", &user, &store).expect("remove");
    assert!(!user.join("sample.rbplugin").exists());
    assert!(store.installs().is_empty());
    assert!(matches!(
        remove("../escape", &user, &store),
        Err(RepositoryError::NotInstalledHere)
    ));
}
