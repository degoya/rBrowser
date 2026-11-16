use super::*;
use std::sync::Arc;

use crate::trust::{TRUST_FILE, TRUST_STORE_VERSION, TrustStore};

#[test]
fn a_missing_store_is_empty() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = load(&dir.path().join(TRUST_FILE)).expect("loads");
    assert_eq!(store.version, TRUST_STORE_VERSION);
    assert!(store.records.is_empty());
}

#[test]
fn an_update_writes_the_store_and_a_grant_replaces_the_roots_record() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("nested").join(TRUST_FILE);
    update(&path, |store| {
        store.grant(Path::new("C:/work/shop"), "aa", "2026-11-10T12:00:00Z");
    })
    .expect("writes");
    let written = update(&path, |store| {
        store.grant(Path::new("C:\\work\\shop\\"), "bb", "2026-11-11T12:00:00Z");
        store.grant(Path::new("C:/work/blog"), "cc", "2026-11-11T12:00:00Z");
    })
    .expect("writes");
    let loaded = load(&path).expect("loads");
    assert_eq!(loaded, written);
    assert_eq!(loaded.records.len(), 2, "{loaded:?}");
    let shop = loaded.record(Path::new("C:/work/shop")).expect("shop");
    assert_eq!(
        (shop.hash.as_str(), shop.root.as_str()),
        ("bb", "C:\\work\\shop\\")
    );
    let text = std::fs::read_to_string(&path).expect("reads");
    assert!(text.contains("\"decidedAt\""), "{text}");
}

#[test]
fn roots_compare_without_separator_and_trailing_slash_differences() {
    let mut store = TrustStore::default();
    store.grant(
        Path::new("\\\\wsl.localhost\\Ubuntu\\home\\me\\shop"),
        "aa",
        "t",
    );
    assert!(
        store
            .record(Path::new("//wsl.localhost/Ubuntu/home/me/shop/"))
            .is_some()
    );
    assert!(
        store
            .record(Path::new("//wsl.localhost/Ubuntu/home/me/shop2"))
            .is_none()
    );
    assert!(store.revoke(Path::new("//wsl.localhost/Ubuntu/home/me/shop")));
    assert!(store.records.is_empty());
}

#[test]
fn a_newer_store_is_refused_and_not_overwritten() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(TRUST_FILE);
    std::fs::write(&path, r#"{ "version": 99, "records": [] }"#).expect("write");
    assert!(matches!(
        load(&path),
        Err(ConfigError::UnsupportedVersion { found: 99, .. })
    ));
    assert!(update(&path, |store| store.records.clear()).is_err());
    assert!(
        std::fs::read_to_string(&path)
            .expect("reads")
            .contains("99")
    );
}

#[test]
fn a_broken_store_is_an_error_not_a_trust() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(TRUST_FILE);
    std::fs::write(&path, "{ not json").expect("write");
    assert!(load(&path).is_err());
    std::fs::write(&path, r#"{ "version": 1, "records": [{ "root": 1 }] }"#).expect("write");
    assert!(load(&path).is_err());
}

#[test]
fn concurrent_updates_lose_no_decision() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = Arc::new(dir.path().join(TRUST_FILE));
    let threads: Vec<_> = (0..8)
        .map(|n| {
            let path = path.clone();
            std::thread::spawn(move || {
                update(&path, |store| {
                    store.grant(Path::new(&format!("C:/work/p{n}")), "aa", "t");
                })
                .expect("writes");
            })
        })
        .collect();
    for thread in threads {
        thread.join().expect("joins");
    }
    assert_eq!(load(&path).expect("loads").records.len(), 8);
}
