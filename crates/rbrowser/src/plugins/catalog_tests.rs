use rb_config::{CatalogRepository, TrustedPluginKey, UserSettings};
use rb_ipc::{NotificationLevel, PluginGrantDetails};
use rb_plugin_host::{OFFICIAL_REPOSITORY, PinnedKey};
use serde_json::json;

use super::{
    CATALOG_SETTING, CHECK_FAILED_NOTICE_KEY, ConsentRequest, FOREIGN_CODE, InstallRequest,
    NO_UPDATES_NOTICE_KEY, UPDATES_NOTICE_KEY, answer, notice, refuse_foreign, repositories,
    same_grants, with_pinned, without_query,
};

/// A base64 Ed25519 public key that decodes: the compiled-in release key's.
fn public_key() -> String {
    rb_plugin_host::EMBEDDED_KEYS[0].public_key.to_owned()
}

fn with_repositories(repositories: Vec<CatalogRepository>) -> UserSettings {
    let mut user = UserSettings::default();
    user.plugins.catalog.repositories = repositories;
    user
}

fn repository(url: &str, key: &str) -> CatalogRepository {
    CatalogRepository {
        name: "Mine".to_owned(),
        url: url.to_owned(),
        public_key: key.to_owned(),
    }
}

#[test]
fn the_official_catalogue_comes_first_and_a_person_s_repositories_follow_once() {
    let key = public_key();
    let user = with_repositories(vec![
        repository("https://mine.test/rbrowser-plugin-index.json", &key),
        repository("https://mine.test/rbrowser-plugin-index.json", &key),
        repository("http://plain.test/rbrowser-plugin-index.json", &key),
        repository("https://bad-key.test/rbrowser-plugin-index.json", "no key"),
    ]);
    let (list, refused) = repositories(&user);
    assert_eq!(list[0].id, OFFICIAL_REPOSITORY);
    assert_eq!(list.len(), 2, "the same address and key count once");
    assert!(list[1].id.starts_with("user-"));
    let errors: Vec<&str> = refused
        .iter()
        .filter_map(|entry| entry["error"].as_str())
        .collect();
    assert_eq!(
        errors,
        [
            "plugins.catalog.error.invalid_url",
            "plugins.catalog.error.invalid_key"
        ]
    );
}

#[test]
fn an_answer_without_a_refresh_offers_what_the_cache_holds_and_reaches_no_network() {
    let dir = tempfile::tempdir().expect("tempdir");
    let answer = answer(
        &UserSettings::default(),
        dir.path().join("plugin-catalog"),
        None,
        json!({ "seo": { "version": "1.0.0", "source": "bundled" } }),
    );
    assert_eq!(answer["repositories"][0]["id"], OFFICIAL_REPOSITORY);
    assert_eq!(answer["repositories"][0]["official"], true);
    assert_eq!(answer["repositories"][0]["loaded"], false);
    assert!(answer["repositories"][0]["error"].is_null());
    assert_eq!(answer["entries"], json!([]));
    assert_eq!(answer["updates"], json!([]));
    assert_eq!(answer["installed"]["seo"]["source"], "bundled");
}

#[test]
fn a_query_in_a_repository_address_never_reaches_an_answer() {
    assert_eq!(
        without_query("https://mine.test/index.json?token=secret#x"),
        "https://mine.test/index.json"
    );
    assert_eq!(without_query("not a url?token=secret"), "not a url");
    // L1: credentials never reach an answer either.
    assert_eq!(
        without_query("https://user:secret@mine.test/index.json"),
        "https://mine.test/index.json"
    );
    let user = with_repositories(vec![repository(
        "http://plain.test/index.json?token=secret",
        &public_key(),
    )]);
    let (_, refused) = repositories(&user);
    assert!(!refused[0].to_string().contains("secret"));
}

#[test]
fn a_confirmed_key_is_pinned_once() {
    let pin = PinnedKey {
        key_id: "author-v1".to_owned(),
        public_key: public_key(),
        fingerprint: "ab".repeat(32),
    };
    let mut user = UserSettings::default();
    let keys = with_pinned(&user, pin.clone());
    assert_eq!(keys.len(), 1);
    user.plugins.trusted_keys = keys;
    let again = with_pinned(&user, pin);
    assert_eq!(
        again,
        [TrustedPluginKey {
            key_id: "author-v1".to_owned(),
            public_key: public_key(),
            fingerprint: "ab".repeat(32),
        }]
    );
}

#[test]
fn the_check_notice_names_the_updates_or_the_failure() {
    let updates = notice(&json!({ "updates": [{}, {}], "repositories": [] }));
    assert_eq!(updates.message_key, UPDATES_NOTICE_KEY);
    assert_eq!(updates.args["count"], "2");
    assert_eq!(updates.settings_key.as_deref(), Some(CATALOG_SETTING));
    let none = notice(&json!({ "updates": [], "repositories": [{ "error": null }] }));
    assert_eq!(none.message_key, NO_UPDATES_NOTICE_KEY);
    let failed = notice(&json!({
        "updates": [],
        "repositories": [{ "error": "plugins.catalog.error.expired" }],
    }));
    assert_eq!(failed.message_key, CHECK_FAILED_NOTICE_KEY);
    assert_eq!(failed.level, NotificationLevel::Warning);
    assert!(failed.args.is_empty(), "a sentence, no error code");
}

#[test]
fn an_install_request_reads_the_dialog_s_consent_and_nothing_else() {
    let request: InstallRequest = serde_json::from_value(json!({
        "repository": "official",
        "id": "seo",
        "version": "1.1.0",
        "consent": {
            "capabilities": ["views"],
            "grants": { "evaluate": false, "cdpDomains": [], "projectGlobs": [],
                        "sidecarMethods": [], "risky": [] },
            "keyFingerprint": null,
        },
    }))
    .expect("parses");
    assert_eq!(
        request.consent,
        ConsentRequest {
            capabilities: vec!["views".to_owned()],
            grants: PluginGrantDetails::default(),
            key_fingerprint: None,
        }
    );
    let extra = serde_json::from_value::<InstallRequest>(json!({
        "repository": "official",
        "id": "seo",
        "version": "1.1.0",
        "consent": { "capabilities": [], "grants": PluginGrantDetails::default() },
        "trustRepository": true,
    }));
    assert!(extra.is_err());
    let without_grants = serde_json::from_value::<InstallRequest>(json!({
        "repository": "official",
        "id": "seo",
        "version": "1.1.0",
        "consent": { "capabilities": ["views"] },
    }));
    assert!(
        without_grants.is_err(),
        "the full grants are part of the consent (M1)"
    );
}

#[test]
fn the_grants_shown_must_be_the_package_s_in_full() {
    let actual = PluginGrantDetails {
        cdp_domains: vec!["Network".to_owned(), "DOM".to_owned()],
        project_globs: vec!["src/**".to_owned()],
        risky: vec!["cdp:DOM".to_owned(), "cdp:Network".to_owned()],
        ..PluginGrantDetails::default()
    };
    let reordered = PluginGrantDetails {
        cdp_domains: vec!["DOM".to_owned(), "Network".to_owned()],
        risky: vec!["cdp:Network".to_owned(), "cdp:DOM".to_owned()],
        ..actual.clone()
    };
    assert!(same_grants(&actual, &reordered));
    let fewer = PluginGrantDetails {
        cdp_domains: vec!["DOM".to_owned()],
        ..actual.clone()
    };
    assert!(!same_grants(&actual, &fewer));
    let window_image = PluginGrantDetails {
        window_image: true,
        ..actual.clone()
    };
    assert!(!same_grants(&actual, &window_image));
}

/// RB-091-04: the catalogue installs and updates a bundled plugin from a package of the bundle's
/// key; one with a bundled plugin's id under another key is refused.
#[test]
fn a_bundled_plugin_installs_only_under_the_bundle_s_key() {
    let bundled: std::collections::BTreeMap<String, String> =
        [("seo".to_owned(), "a".repeat(64))].into();
    assert!(refuse_foreign("seo", &"a".repeat(64), &bundled).is_ok());
    let refused = refuse_foreign("seo", &"b".repeat(64), &bundled).expect_err("another key");
    assert_eq!(refused.1, FOREIGN_CODE);
    assert!(refuse_foreign("todo", &"b".repeat(64), &bundled).is_ok());
}

/// The status bar counts the bundled plugins' updates with the catalogue's.
#[test]
fn the_notice_counts_the_bundled_plugins_updates() {
    let answer = json!({
        "repositories": [],
        "updates": [],
        "bundled": [
            { "id": "seo", "update": { "repository": "official", "version": "2.0.0" } },
            { "id": "todo", "update": null }
        ]
    });
    let notice = notice(&answer);
    assert_eq!(notice.message_key, UPDATES_NOTICE_KEY);
    assert_eq!(notice.args["count"], "1");
}
