use super::{CatalogStore, InstallRecord};

#[test]
fn a_floor_only_rises() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = CatalogStore::new(dir.path().join("plugin-catalog"));
    assert!(store.floors().is_empty());
    store.raise_floor("official", 7).expect("raise");
    store.raise_floor("official", 3).expect("not lowered");
    assert_eq!(store.floors().get("official"), Some(&7));
}

#[test]
fn the_cached_index_and_the_installs_round_trip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = CatalogStore::new(dir.path().to_path_buf());
    store.cache_index("user-0123", b"{}").expect("cache");
    assert_eq!(store.cached_index("user-0123").as_deref(), Some(&b"{}"[..]));
    store.drop_index("user-0123");
    assert!(store.cached_index("user-0123").is_none());
    let record = InstallRecord {
        repository: "official".to_owned(),
        version: "1.0.0".to_owned(),
        sha256: "00".repeat(32),
        fingerprint: Some("ab".repeat(32)),
    };
    store
        .record_install("seo", Some(record.clone()))
        .expect("record");
    assert_eq!(store.installs().get("seo"), Some(&record));
    store.record_install("seo", None).expect("forget");
    assert!(store.installs().is_empty());
    let staging: Vec<_> = std::fs::read_dir(dir.path())
        .expect("listed")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.ends_with(".tmp") || name.ends_with(".partial"))
        .collect();
    assert!(staging.is_empty(), "no staging file is left: {staging:?}");
}

#[test]
fn a_file_that_does_not_read_counts_as_absent() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("floors.json"), b"not json").expect("write");
    let store = CatalogStore::new(dir.path().to_path_buf());
    assert!(store.floors().is_empty());
}
