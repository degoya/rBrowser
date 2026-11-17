use super::*;
use serde_json::json;
use std::fs;

/// The prepared repository of K9, `tests/fixtures/untrusted-project/`.
fn untrusted_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/untrusted-project")
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("mkdir");
    for entry in fs::read_dir(from).expect("read dir") {
        let entry = entry.expect("entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy");
        }
    }
}

/// A copy of the K9 fixture in a temp directory (tests change it).
fn untrusted_copy() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    copy_dir(&untrusted_fixture(), dir.path());
    dir
}

fn edit_config(root: &Path, file: &str, change: impl FnOnce(&mut serde_json::Value)) {
    let path = root.join(".rbrowser").join(file);
    let mut value = crate::read_json(&path).expect("reads");
    change(&mut value);
    crate::write_json(&path, &value).expect("writes");
}

fn kinds(state: &TrustState) -> Vec<(ProtectedKind, &str)> {
    state
        .blocked
        .iter()
        .map(|item| (item.kind, item.path.as_str()))
        .collect()
}

fn trusting(root: &Path, state: &TrustState) -> TrustStore {
    let mut store = TrustStore::default();
    store.grant(root, &state.hash, "2026-11-10T12:00:00Z");
    store
}

#[test]
fn an_untrusted_state_allows_nothing_protected() {
    let state = TrustState {
        trusted: false,
        hash: "ab".repeat(32),
        blocked: vec![BlockedItem {
            kind: ProtectedKind::Headers,
            path: "/emulation/headers".to_owned(),
        }],
        unpinned: Vec::new(),
    };
    assert!(!state.allows(ProtectedKind::Headers));
    assert!(!state.allows(ProtectedKind::Flows));
    let trusted = TrustState {
        trusted: true,
        ..state
    };
    assert!(trusted.allows(ProtectedKind::Headers));
}

#[test]
fn the_wire_form_is_camel_case() {
    let item = BlockedItem {
        kind: ProtectedKind::HostResolverRules,
        path: "/network/hostResolverRules".to_owned(),
    };
    assert_eq!(
        serde_json::to_value(&item).expect("serializes"),
        json!({ "kind": "hostResolverRules", "path": "/network/hostResolverRules" })
    );
    assert_eq!(
        serde_json::to_value(ProtectedKind::IgnoreCertificateErrors).expect("serializes"),
        json!("ignoreCertificateErrors")
    );
    let record = TrustRecord {
        root: "C:/work/shop".to_owned(),
        hash: String::new(),
        decided_at: "2026-11-10T12:00:00Z".to_owned(),
    };
    assert_eq!(
        serde_json::to_value(&record).expect("serializes")["decidedAt"],
        "2026-11-10T12:00:00Z"
    );
}

#[test]
fn the_store_lives_beside_the_user_settings() {
    assert_eq!(
        trust_store_path(),
        crate::user_settings_path().map(|settings| trust_store_for(&settings))
    );
    assert_eq!(
        trust_store_for(Path::new("/data/rbrowser/settings.json")),
        Path::new("/data/rbrowser").join(TRUST_FILE)
    );
}

/// K9: every protected part of the prepared repository is listed and stays blocked; the
/// harmless parts load.
#[test]
fn the_untrusted_fixture_blocks_every_protected_part_and_loads_the_rest() {
    let dir = untrusted_copy();
    let (loaded, state) = load_project_trusted(dir.path(), &TrustStore::default()).expect("loads");
    assert!(!state.trusted);
    assert_eq!(state.hash.len(), 64, "a SHA-256 in hex");
    assert_eq!(
        kinds(&state),
        [
            (ProtectedKind::Flows, ".rbrowser/flows/login.ts"),
            (ProtectedKind::Snippets, ".rbrowser/snippets/title.js"),
            (ProtectedKind::Plugins, ".rbrowser/plugins/greeter"),
            (ProtectedKind::Mocks, ".rbrowser/mocks/cart.json"),
            (ProtectedKind::Headers, "/breakpoints/1/headers"),
            (ProtectedKind::Headers, "/emulation/headers"),
            (
                ProtectedKind::HostResolverRules,
                ".rbrowser/config.local.json#/network/hostResolverRules"
            ),
            (
                ProtectedKind::HostResolverRules,
                "/network/hostResolverRules"
            ),
            (ProtectedKind::PageScripts, ".rbrowser/scripts/inject.js"),
            (ProtectedKind::Extensions, "/extensions"),
            (
                ProtectedKind::IgnoreCertificateErrors,
                ".rbrowser/config.local.json#/network/ignoreCertificateErrors"
            ),
            (
                ProtectedKind::Proxy,
                ".rbrowser/config.local.json#/network/proxy"
            ),
        ]
    );
    for kind in [
        ProtectedKind::Flows,
        ProtectedKind::Snippets,
        ProtectedKind::Plugins,
        ProtectedKind::Mocks,
        ProtectedKind::Headers,
        ProtectedKind::HostResolverRules,
        ProtectedKind::IgnoreCertificateErrors,
        ProtectedKind::Proxy,
        ProtectedKind::PageScripts,
        ProtectedKind::Extensions,
    ] {
        assert!(!state.allows(kind), "{kind:?}");
    }

    // The network overrides of both files stay out of the effective config.
    let config = &loaded.config;
    assert_eq!(config.network, crate::NetworkConfig::default());
    let gated: Vec<&str> = loaded
        .warnings
        .iter()
        .filter(|w| w.key == "config.warning.trust_required")
        .map(|w| w.path.as_str())
        .collect();
    assert_eq!(gated.len(), 4, "{:?}", loaded.warnings);

    // Breakpoints, layout and environments load.
    assert_eq!(config.breakpoints.len(), 2);
    assert_eq!(config.layout.cells.len(), 2);
    assert_eq!(config.environments.len(), 1);
}

#[test]
fn trusting_applies_the_network_keys_of_both_files() {
    let dir = untrusted_copy();
    let state = trust_state(dir.path(), &TrustStore::default()).expect("state");
    let store = trusting(dir.path(), &state);
    let (loaded, after) = load_project_trusted(dir.path(), &store).expect("loads");
    assert!(after.trusted);
    assert_eq!(after.hash, state.hash);
    assert_eq!(after.blocked, state.blocked, "the parts stay listed");
    assert!(after.allows(ProtectedKind::Proxy));
    let network = &loaded.config.network;
    assert!(network.ignore_certificate_errors);
    assert_eq!(network.proxy.as_deref(), Some("http://203.0.113.7:8080"));
    assert_eq!(
        network.host_resolver_rules.as_deref(),
        Some("MAP shop.test 203.0.113.7"),
        "config.local.json wins the merge"
    );
    assert!(
        !loaded
            .warnings
            .iter()
            .any(|w| w.key == "config.warning.trust_required")
    );
}

#[test]
fn a_changed_flow_asks_again_but_a_changed_breakpoint_does_not() {
    let dir = untrusted_copy();
    let (_, state) = load_project_trusted(dir.path(), &TrustStore::default()).expect("loads");
    let store = trusting(dir.path(), &state);

    edit_config(dir.path(), "config.json", |config| {
        config["breakpoints"][0]["width"] = json!(1600);
        let first = config["breakpoints"][0].take();
        let second = config["breakpoints"][1].take();
        config["breakpoints"] = json!([second, first]);
        config["layout"]["gap"] = json!(4);
    });
    let (_, after) = load_project_trusted(dir.path(), &store).expect("loads");
    assert!(
        after.trusted,
        "breakpoints, their order and the layout are not protected"
    );
    assert_eq!(after.hash, state.hash);

    fs::write(
        dir.path().join(".rbrowser/flows/login.ts"),
        "await page.goto('https://example.invalid/other');\n",
    )
    .expect("write");
    let (loaded, changed) = load_project_trusted(dir.path(), &store).expect("loads");
    assert!(!changed.trusted, "a changed flow asks again");
    assert_ne!(changed.hash, state.hash);
    assert_eq!(loaded.config.network, crate::NetworkConfig::default());
}

#[test]
fn every_protected_change_asks_again() {
    type Change = fn(&Path);
    let changes: [(&str, Change); 7] = [
        ("a new mock", |root| {
            fs::write(root.join(".rbrowser/mocks/new.json"), "{}").expect("write");
        }),
        ("a plugin file", |root| {
            fs::write(
                root.join(".rbrowser/plugins/greeter/plugin.wasm"),
                [0u8, 97],
            )
            .expect("w");
        }),
        ("a removed snippet", |root| {
            fs::remove_file(root.join(".rbrowser/snippets/title.js")).expect("remove");
        }),
        ("a project header", |root| {
            edit_config(root, "config.json", |c| {
                c["emulation"]["headers"]["X-Untrusted"] = json!("changed");
            });
        }),
        ("a breakpoint header", |root| {
            edit_config(root, "config.json", |c| {
                c["breakpoints"][0]["headers"] = json!({ "X-New": "1" });
            });
        }),
        ("the local proxy", |root| {
            edit_config(root, "config.local.json", |c| {
                c["network"]["proxy"] = json!("http://198.51.100.1:3128");
            });
        }),
        ("an extension", |root| {
            edit_config(root, "config.json", |c| {
                c["extensions"] = json!([{ "id": "pppppppppppppppppppppppppppppppp" }]);
            });
        }),
    ];
    for (what, change) in changes {
        let dir = untrusted_copy();
        let (_, state) = load_project_trusted(dir.path(), &TrustStore::default()).expect("loads");
        let store = trusting(dir.path(), &state);
        change(dir.path());
        let (_, after) = load_project_trusted(dir.path(), &store).expect("loads");
        assert!(!after.trusted, "{what} must ask again");
    }
}

#[test]
fn trust_belongs_to_one_path() {
    let dir = untrusted_copy();
    let (_, state) = load_project_trusted(dir.path(), &TrustStore::default()).expect("loads");
    let store = trusting(dir.path(), &state);
    let other = untrusted_copy();
    let (_, elsewhere) = load_project_trusted(other.path(), &store).expect("loads");
    assert_eq!(elsewhere.hash, state.hash, "the same content");
    assert!(!elsewhere.trusted, "but another path");
}

#[test]
fn revoking_blocks_the_parts_again() {
    let dir = untrusted_copy();
    let (_, state) = load_project_trusted(dir.path(), &TrustStore::default()).expect("loads");
    let mut store = trusting(dir.path(), &state);
    assert!(store.revoke(dir.path()));
    assert!(!store.revoke(dir.path()), "nothing left to revoke");
    let (loaded, after) = load_project_trusted(dir.path(), &store).expect("loads");
    assert!(!after.trusted);
    assert_eq!(loaded.config.network, crate::NetworkConfig::default());
}

#[test]
fn a_project_without_protected_parts_has_an_empty_hash() {
    let dir = untrusted_copy();
    for sub in ["flows", "plugins", "mocks", "snippets", "scripts"] {
        fs::remove_dir_all(dir.path().join(".rbrowser").join(sub)).expect("remove");
    }
    fs::remove_file(dir.path().join(".rbrowser/config.local.json")).expect("remove");
    edit_config(dir.path(), "config.json", |c| {
        let object = c.as_object_mut().expect("object");
        for key in ["emulation", "network", "extensions"] {
            object.remove(key);
        }
        c["breakpoints"][1]
            .as_object_mut()
            .expect("object")
            .remove("headers");
    });
    let (_, state) = load_project_trusted(dir.path(), &TrustStore::default()).expect("loads");
    assert!(state.blocked.is_empty());
    assert_eq!(state.hash, "");
    assert!(!state.trusted, "nothing was decided");
}

#[test]
fn the_local_only_keys_of_config_json_are_not_offered_for_trust() {
    let dir = untrusted_copy();
    fs::remove_file(dir.path().join(".rbrowser/config.local.json")).expect("remove");
    edit_config(dir.path(), "config.json", |c| {
        c["network"]["proxy"] = json!("http://203.0.113.7:8080");
        c["network"]["ignoreCertificateErrors"] = json!(true);
    });
    let (_, state) = load_project_trusted(dir.path(), &TrustStore::default()).expect("loads");
    let store = trusting(dir.path(), &state);
    assert!(
        !state.blocked.iter().any(|item| matches!(
            item.kind,
            ProtectedKind::Proxy | ProtectedKind::IgnoreCertificateErrors
        )),
        "{:?}",
        state.blocked
    );
    let (loaded, after) = load_project_trusted(dir.path(), &store).expect("loads");
    assert!(after.trusted);
    assert_eq!(
        loaded.config.network.proxy, None,
        "local-only stays local-only"
    );
    assert!(!loaded.config.network.ignore_certificate_errors);
}

#[test]
fn harmless_values_of_the_gated_keys_are_not_listed() {
    let dir = untrusted_copy();
    fs::write(
        dir.path().join(".rbrowser/config.local.json"),
        r#"{ "network": { "ignoreCertificateErrors": false, "proxy": null, "hostResolverRules": "" } }"#,
    )
    .expect("write");
    let (_, state) = load_project_trusted(dir.path(), &TrustStore::default()).expect("loads");
    assert!(
        !state
            .blocked
            .iter()
            .any(|item| item.path.starts_with(".rbrowser/config.local.json")),
        "{:?}",
        state.blocked
    );
}

#[cfg(unix)]
#[test]
fn a_link_inside_the_project_is_hashed_by_the_content_it_leads_to() {
    let dir = untrusted_copy();
    fs::create_dir_all(dir.path().join("src")).expect("mkdir");
    fs::write(dir.path().join("src/shared.ts"), "one").expect("write");
    std::os::unix::fs::symlink(
        dir.path().join("src/shared.ts"),
        dir.path().join(".rbrowser/flows/linked.ts"),
    )
    .expect("link");
    let (_, state) = load_project_trusted(dir.path(), &TrustStore::default()).expect("loads");
    assert!(state.unpinned.is_empty(), "{:?}", state.unpinned);
    assert!(
        state
            .blocked
            .iter()
            .any(|item| item.path == ".rbrowser/flows/linked.ts")
    );
    let store = trusting(dir.path(), &state);
    let (_, trusted) = load_project_trusted(dir.path(), &store).expect("loads");
    assert!(trusted.trusted);
    fs::write(dir.path().join("src/shared.ts"), "two").expect("write");
    let (_, again) = load_project_trusted(dir.path(), &store).expect("loads");
    assert_ne!(again.hash, state.hash, "the content behind the link counts");
    assert!(!again.trusted, "a changed target asks again");
}

#[cfg(unix)]
#[test]
fn a_link_leading_out_of_the_project_cannot_be_trusted() {
    let dir = untrusted_copy();
    let outside = tempfile::tempdir().expect("temp dir");
    fs::write(outside.path().join("big.ts"), "one").expect("write");
    std::os::unix::fs::symlink(
        outside.path().join("big.ts"),
        dir.path().join(".rbrowser/flows/linked.ts"),
    )
    .expect("link");
    std::os::unix::fs::symlink(
        dir.path().join("missing.ts"),
        dir.path().join(".rbrowser/mocks/dangling.json"),
    )
    .expect("link");
    let (_, state) = load_project_trusted(dir.path(), &TrustStore::default()).expect("loads");
    assert_eq!(
        state.unpinned,
        vec![
            ".rbrowser/flows/linked.ts".to_owned(),
            ".rbrowser/mocks/dangling.json".to_owned(),
        ]
    );
    let store = trusting(dir.path(), &state);
    let (_, again) = load_project_trusted(dir.path(), &store).expect("loads");
    assert!(!again.trusted, "a record cannot cover what a link leads to");
    let direct = trust_state(dir.path(), &store).expect("state");
    assert!(!direct.trusted);
    assert!(!direct.allows(ProtectedKind::Flows));
}

#[cfg(unix)]
#[test]
fn a_link_back_into_its_own_folder_does_not_loop() {
    let dir = untrusted_copy();
    std::os::unix::fs::symlink(
        dir.path().join(".rbrowser/flows"),
        dir.path().join(".rbrowser/flows/again"),
    )
    .expect("link");
    std::os::unix::fs::symlink(dir.path(), dir.path().join(".rbrowser/scripts/root"))
        .expect("link");
    let (_, state) = load_project_trusted(dir.path(), &TrustStore::default()).expect("loads");
    assert!(state.unpinned.is_empty(), "{:?}", state.unpinned);
    assert_eq!(state.hash.len(), 64);
}

#[test]
fn a_known_trust_loads_like_the_trusted_loader_without_a_store() {
    let dir = untrusted_copy();
    let (_, state) = load_project_trusted(dir.path(), &TrustStore::default()).expect("loads");
    let (trusted, _) =
        load_project_trusted(dir.path(), &trusting(dir.path(), &state)).expect("loads");
    let known = load_project_with_trust(dir.path(), true).expect("loads");
    assert_eq!(known.config, trusted.config);
    let untrusted = load_project_with_trust(dir.path(), false).expect("loads");
    assert_eq!(untrusted.config.network, crate::NetworkConfig::default());
}

#[test]
fn a_large_protected_file_is_hashed_like_its_bytes() {
    let dir = untrusted_copy();
    let flow = dir.path().join(".rbrowser/flows/login.ts");
    let big = vec![b'x'; 200 * 1024 + 7];
    fs::write(&flow, &big).expect("write");
    let (_, before) = load_project_trusted(dir.path(), &TrustStore::default()).expect("loads");
    let mut changed = big.clone();
    changed[150 * 1024] = b'y';
    fs::write(&flow, &changed).expect("write");
    let (_, after) = load_project_trusted(dir.path(), &TrustStore::default()).expect("loads");
    assert_ne!(
        before.hash, after.hash,
        "a byte past the first buffer counts"
    );
}

#[test]
fn trust_state_answers_like_the_trusted_loader() {
    let dir = untrusted_copy();
    let (_, state) = load_project_trusted(dir.path(), &TrustStore::default()).expect("loads");
    let store = trusting(dir.path(), &state);
    let direct = trust_state(dir.path(), &store).expect("state");
    assert!(direct.trusted);
    assert_eq!(direct.hash, state.hash);
}
