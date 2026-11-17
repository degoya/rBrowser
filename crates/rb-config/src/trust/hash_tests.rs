use super::*;
use serde_json::json;

fn raw(project: Value, local: Option<Value>) -> RawFiles {
    RawFiles {
        project,
        local,
        migrations: Vec::new(),
    }
}

#[test]
fn the_hash_ignores_key_order_and_unprotected_values() {
    let dir = tempfile::tempdir().expect("temp dir");
    let a = protected(
        dir.path(),
        &raw(
            json!({ "version": 1, "emulation": { "headers": { "A": "1", "B": "2" } } }),
            None,
        ),
    )
    .expect("collects");
    let b = protected(
        dir.path(),
        &raw(
            json!({ "emulation": { "dpr": 2, "headers": { "B": "2", "A": "1" } }, "version": 1 }),
            None,
        ),
    )
    .expect("collects");
    assert_eq!(a.hash, b.hash);
    assert_eq!(a.hash.len(), 64);
    let c = protected(
        dir.path(),
        &raw(
            json!({ "emulation": { "headers": { "A": "1", "B": "3" } } }),
            None,
        ),
    )
    .expect("collects");
    assert_ne!(a.hash, c.hash);
}

#[test]
fn the_same_value_in_the_other_file_is_another_part() {
    let dir = tempfile::tempdir().expect("temp dir");
    let rules = json!({ "network": { "hostResolverRules": "MAP * 10.0.0.1" } });
    let shared = protected(dir.path(), &raw(rules.clone(), None)).expect("collects");
    let local = protected(dir.path(), &raw(json!({}), Some(rules))).expect("collects");
    assert_ne!(shared.hash, local.hash);
}

#[test]
fn every_gated_key_has_a_kind() {
    for key in TRUST_GATED_KEYS {
        assert!(gated_kind(key).is_some(), "{key}");
    }
}

#[test]
fn a_file_named_like_a_protected_folder_is_protected_too() {
    let dir = tempfile::tempdir().expect("temp dir");
    fs::create_dir_all(dir.path().join(CONFIG_DIR)).expect("mkdir");
    fs::write(dir.path().join(CONFIG_DIR).join("flows"), "x").expect("write");
    let found = protected(dir.path(), &raw(json!({}), None)).expect("collects");
    assert_eq!(
        found.blocked,
        [BlockedItem {
            kind: ProtectedKind::Flows,
            path: ".rbrowser/flows".to_owned()
        }]
    );
    assert!(!found.hash.is_empty());
}

#[test]
fn an_empty_protected_folder_blocks_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    fs::create_dir_all(dir.path().join(CONFIG_DIR).join("mocks")).expect("mkdir");
    let found = protected(dir.path(), &raw(json!({}), None)).expect("collects");
    assert!(found.blocked.is_empty());
    assert_eq!(found.hash, "");
}

#[test]
fn a_nested_file_changes_the_hash_but_lists_its_top_folder() {
    let dir = tempfile::tempdir().expect("temp dir");
    let plugin = dir.path().join(CONFIG_DIR).join("plugins/greeter/locales");
    fs::create_dir_all(&plugin).expect("mkdir");
    fs::write(plugin.join("en.json"), "{}").expect("write");
    let before = protected(dir.path(), &raw(json!({}), None)).expect("collects");
    assert_eq!(before.blocked.len(), 1);
    assert_eq!(before.blocked[0].path, ".rbrowser/plugins/greeter");
    fs::write(plugin.join("en.json"), "{ }").expect("write");
    let after = protected(dir.path(), &raw(json!({}), None)).expect("collects");
    assert_ne!(before.hash, after.hash);
}

#[test]
fn the_headers_of_the_local_file_are_protected() {
    let dir = tempfile::tempdir().expect("temp dir");
    let local = json!({
        "emulation": { "headers": { "Authorization": "Bearer a" } },
        "breakpoints": [{ "id": "mobile", "headers": { "Cookie": "a=1" } }]
    });
    let found = protected(dir.path(), &raw(json!({}), Some(local.clone()))).expect("collects");
    assert_eq!(
        found.blocked,
        [
            BlockedItem {
                kind: ProtectedKind::Headers,
                path: ".rbrowser/config.local.json#/breakpoints/0/headers".to_owned()
            },
            BlockedItem {
                kind: ProtectedKind::Headers,
                path: ".rbrowser/config.local.json#/emulation/headers".to_owned()
            },
        ]
    );
    let mut changed = local.clone();
    changed["breakpoints"][0]["headers"]["Cookie"] = json!("a=2");
    let after = protected(dir.path(), &raw(json!({}), Some(changed))).expect("collects");
    assert_ne!(found.hash, after.hash, "a changed local header asks again");
    let shared = protected(dir.path(), &raw(local, None)).expect("collects");
    assert_ne!(
        found.hash, shared.hash,
        "the same headers in config.json are another part"
    );
}
