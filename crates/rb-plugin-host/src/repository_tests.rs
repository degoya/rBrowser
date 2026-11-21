//! The catalogue's refresh, cache, merge and updates against served repositories (RB-091-01).

use super::fixtures::{MINE_URL, OFFICIAL_URL, at, folders, mine, official, serve_index};
use super::{Catalog, CatalogStore, InstallRecord, MemoryFetcher, Repository, RepositoryError};
use crate::index::fixtures::{entry, index_of, signed};

fn catalog(store: &CatalogStore, repositories: Vec<Repository>) -> Catalog {
    Catalog::new(store.clone(), repositories)
}

fn codes(outcomes: &[(String, Result<(), RepositoryError>)]) -> Vec<(String, &'static str)> {
    outcomes
        .iter()
        .map(|(id, outcome)| {
            let code = outcome.as_ref().err().map_or("ok", RepositoryError::code);
            (id.clone(), code)
        })
        .collect()
}

#[test]
fn a_refresh_reads_every_repository_and_merges_their_offers() {
    let (_dir, store, _user) = folders();
    let fetcher = MemoryFetcher::new();
    serve_index(&fetcher, OFFICIAL_URL, 3, vec![entry("seo", "1.0.0")]);
    serve_index(
        &fetcher,
        MINE_URL,
        1,
        vec![entry("seo", "2.0.0"), entry("todo", "1.0.0")],
    );
    let mut catalog = catalog(&store, vec![official(), mine()]);
    let outcomes = catalog.refresh(&fetcher, at());
    let mine_id = mine().id;
    assert_eq!(
        codes(&outcomes),
        [("official".to_owned(), "ok"), (mine_id.clone(), "ok")]
    );
    let entries = catalog.entries(at());
    let seo = entries.iter().find(|entry| entry.id == "seo").expect("seo");
    let origins: Vec<&str> = seo
        .offers
        .iter()
        .map(|offer| offer.repository.as_str())
        .collect();
    assert_eq!(origins, ["official", mine_id.as_str()]);
    assert_eq!(store.floors().get("official"), Some(&3));
}

#[test]
fn an_older_index_after_a_newer_one_is_a_replay_and_the_newer_one_stays() {
    let (_dir, store, _user) = folders();
    let fetcher = MemoryFetcher::new();
    serve_index(&fetcher, OFFICIAL_URL, 7, vec![entry("seo", "1.1.0")]);
    let mut catalog = catalog(&store, vec![official()]);
    assert_eq!(codes(&catalog.refresh(&fetcher, at()))[0].1, "ok");
    serve_index(&fetcher, OFFICIAL_URL, 6, vec![entry("seo", "1.0.0")]);
    assert_eq!(
        codes(&catalog.refresh(&fetcher, at()))[0].1,
        "plugins.catalog.error.replayed"
    );
    let entries = catalog.entries(at());
    assert_eq!(entries[0].offers[0].entry.version, "1.1.0");
}

#[test]
fn the_same_bytes_again_are_no_replay() {
    let (_dir, store, _user) = folders();
    let fetcher = MemoryFetcher::new();
    serve_index(&fetcher, OFFICIAL_URL, 4, vec![entry("seo", "1.0.0")]);
    let mut catalog = catalog(&store, vec![official()]);
    for _ in 0..2 {
        assert_eq!(codes(&catalog.refresh(&fetcher, at()))[0].1, "ok");
    }
}

#[test]
fn a_repository_signed_with_another_key_is_refused_and_offers_nothing() {
    let (_dir, store, _user) = folders();
    let fetcher = MemoryFetcher::new();
    let impostor = ed25519_dalek::SigningKey::from_bytes(&[3; 32]);
    let bytes = crate::index::sign(
        "test-repository",
        &impostor,
        &index_of(2, vec![entry("seo", "1.0.0")]),
    )
    .expect("sign");
    fetcher.serve(OFFICIAL_URL, bytes);
    let mut catalog = catalog(&store, vec![official()]);
    assert_eq!(
        codes(&catalog.refresh(&fetcher, at()))[0].1,
        "plugins.catalog.error.bad_signature"
    );
    assert!(catalog.entries(at()).is_empty());
    assert!(store.cached_index("official").is_none());
}

#[test]
fn a_cached_index_is_verified_again_at_start_and_an_expired_one_is_dropped() {
    let (_dir, store, _user) = folders();
    let fetcher = MemoryFetcher::new();
    serve_index(&fetcher, OFFICIAL_URL, 4, vec![entry("seo", "1.0.0")]);
    catalog(&store, vec![official()]).refresh(&fetcher, at());
    let mut restarted = catalog(&store, vec![official()]);
    restarted.load_cached(at());
    assert!(restarted.is_loaded("official", at()));
    let much_later = at() + time::SignedDuration::days(60);
    let mut later = catalog(&store, vec![official()]);
    later.load_cached(much_later);
    assert!(!later.is_loaded("official", much_later));
    assert!(
        store.cached_index("official").is_none(),
        "deleted, not trusted"
    );
}

#[test]
fn a_loaded_index_that_expires_offers_nothing_more() {
    let (_dir, store, _user) = folders();
    let fetcher = MemoryFetcher::new();
    serve_index(&fetcher, OFFICIAL_URL, 4, vec![entry("seo", "1.0.0")]);
    let mut catalog = catalog(&store, vec![official()]);
    catalog.refresh(&fetcher, at());
    assert_eq!(catalog.entries(at()).len(), 1);
    assert!(
        catalog
            .entries(at() + time::SignedDuration::days(60))
            .is_empty()
    );
}

#[test]
fn a_cached_index_below_the_floor_is_refused() {
    let (_dir, store, _user) = folders();
    store
        .cache_index(
            "official",
            &signed(&index_of(2, vec![entry("seo", "1.0.0")])),
        )
        .expect("cache");
    store.raise_floor("official", 5).expect("floor");
    let mut catalog = catalog(&store, vec![official()]);
    catalog.load_cached(at());
    assert!(!catalog.is_loaded("official", at()));
}

#[test]
fn an_update_is_offered_from_the_origin_only() {
    let (_dir, store, _user) = folders();
    let fetcher = MemoryFetcher::new();
    serve_index(&fetcher, OFFICIAL_URL, 4, vec![entry("seo", "1.1.0")]);
    serve_index(&fetcher, MINE_URL, 1, vec![entry("seo", "9.0.0")]);
    store
        .record_install(
            "seo",
            Some(InstallRecord {
                repository: "official".to_owned(),
                version: "1.0.0".to_owned(),
                sha256: "00".repeat(32),
                fingerprint: None,
            }),
        )
        .expect("record");
    let mut catalog = catalog(&store, vec![official(), mine()]);
    catalog.refresh(&fetcher, at());
    let updates = catalog.updates(at());
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].repository, "official");
    assert_eq!(updates[0].entry.version, "1.1.0");
}

#[test]
fn an_offer_names_its_repository_and_version() {
    let (_dir, store, _user) = folders();
    let fetcher = MemoryFetcher::new();
    serve_index(&fetcher, OFFICIAL_URL, 4, vec![entry("seo", "1.0.0")]);
    let mut catalog = catalog(&store, vec![official(), mine()]);
    catalog.refresh(&fetcher, at());
    let (url, offered) = catalog
        .offer("official", "seo", "1.0.0", at())
        .expect("offered");
    assert_eq!(url.as_str(), OFFICIAL_URL);
    assert_eq!(offered.version, "1.0.0");
    let elsewhere = catalog.offer(&mine().id, "seo", "1.0.0", at());
    assert!(matches!(elsewhere, Err(RepositoryError::NotOffered { .. })));
}

#[test]
fn a_user_repository_needs_https_and_a_key() {
    let key = crate::public_key_base64(&crate::index::fixtures::repository_key());
    assert!(matches!(
        Repository::user("x", "http://plain.test/index.json", &key),
        Err(RepositoryError::InvalidUrl)
    ));
    assert!(matches!(
        Repository::user("x", "https://u:p@creds.test/index.json", &key),
        Err(RepositoryError::InvalidUrl)
    ));
    assert!(matches!(
        Repository::user("x", "https://ok.test/index.json", "not a key"),
        Err(RepositoryError::InvalidKey)
    ));
    let repository = Repository::user("", "https://ok.test/index.json", &key).expect("valid");
    assert!(repository.id.starts_with("user-"));
    assert_eq!(repository.id.len(), "user-".len() + 16);
    assert_eq!(repository.name, "ok.test");
    assert!(!repository.is_official());
    assert!(repository.key_fingerprint().is_some());
}

#[test]
fn a_verified_cache_raises_the_floor_even_without_a_floors_file() {
    let (_dir, store, _user) = folders();
    store
        .cache_index(
            "official",
            &signed(&index_of(9, vec![entry("seo", "1.0.0")])),
        )
        .expect("cache");
    assert!(store.floors().is_empty());
    let mut catalog = catalog(&store, vec![official()]);
    catalog.load_cached(at());
    assert_eq!(store.floors().get("official"), Some(&9));
    let fetcher = MemoryFetcher::new();
    serve_index(&fetcher, OFFICIAL_URL, 8, vec![entry("seo", "0.9.0")]);
    assert_eq!(
        codes(&catalog.refresh(&fetcher, at()))[0].1,
        "plugins.catalog.error.replayed"
    );
}
