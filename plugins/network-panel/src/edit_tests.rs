use serde_json::json;

use super::*;
use crate::fake_host::FakeHost;

fn trusted(extra: Value) -> Value {
    let mut args = json!({ "$network": { "session": "s1", "trusted": true,
                                         "hits": { "api.json#user": 4 } } });
    if let (Some(args), Value::Object(extra)) = (args.as_object_mut(), extra) {
        args.extend(extra);
    }
    args
}

fn host() -> FakeHost {
    let mut host = FakeHost::default();
    host.file(
        ".rbrowser/mocks/api.json",
        r#"{"rules":[{"id":"user","match":{"urlPattern":"*/api/user","method":"GET"},
            "mock":{"status":500,"delayMs":800}}]}"#,
    );
    host
}

#[test]
fn mock_add_needs_trust_and_names_why() {
    let mut host = host();
    let refused = add(
        &mut host,
        &json!({ "match": { "urlPattern": "*" }, "block": true }),
    )
    .expect_err("untrusted");
    assert_eq!(
        refused.code.as_deref(),
        Some("plugin.network-panel.errors.untrusted")
    );
    assert!(host.out("mocks/added.json").is_none());
}

#[test]
fn mock_add_writes_the_plugins_own_file_and_mock_remove_takes_it_out() {
    let mut host = host();
    let added = add(
        &mut host,
        &trusted(
            json!({ "match": { "urlPattern": "*/api/user", "method": "GET" },
                         "mock": { "status": 500, "delayMs": 800, "body": { "error": 1 } } }),
        ),
    )
    .expect("added");
    assert_eq!(added["id"], "own:added.json#rule-1");
    let second = add(
        &mut host,
        &trusted(json!({ "id": "ads", "match": { "urlPattern": "*/ads/*" },
                                                 "block": true })),
    )
    .expect("added");
    assert_eq!(second["id"], "own:added.json#ads");
    let file = host.out("mocks/added.json").expect("written");
    assert_eq!(
        mocks::parse_file(file.as_bytes(), "p", "p", true)
            .rules
            .len(),
        2
    );
    let removed = remove(
        &mut host,
        &trusted(json!({ "id": "own:added.json#rule-1" })),
    )
    .expect("removed");
    assert_eq!(removed["removed"], true);
    let file = host.out("mocks/added.json").expect("written");
    assert_eq!(
        mocks::parse_file(file.as_bytes(), "p", "p", true)
            .rules
            .len(),
        1
    );
    let missing = remove(
        &mut host,
        &trusted(json!({ "id": "own:added.json#rule-1" })),
    )
    .expect_err("gone");
    assert_eq!(
        missing.code.as_deref(),
        Some("plugin.network-panel.errors.unknown_rule")
    );
}

#[test]
fn a_bad_rule_is_refused_with_its_reason() {
    let mut host = host();
    let refused = add(
        &mut host,
        &trusted(json!({ "match": { "urlPattern": "*" } })),
    )
    .expect_err("no action");
    assert_eq!(
        refused.code.as_deref(),
        Some("plugin.network-panel.errors.bad_rule")
    );
    assert!(refused.params.contains_key("reason"));
}

#[test]
fn removing_a_project_rule_switches_it_off_and_enable_switches_it_on() {
    let mut host = host();
    let answer =
        remove(&mut host, &trusted(json!({ "id": "api.json#user" }))).expect("switched off");
    assert_eq!(answer["disabled"], true);
    assert!(
        store::read(&mut host, "s1")
            .disabled
            .contains("api.json#user")
    );
    let overview = overview(&mut host, &trusted(json!({})));
    assert_eq!(overview["files"][0]["rules"][0]["enabled"], false);
    assert_eq!(overview["files"][0]["rules"][0]["hits"], 4);
    enable(
        &mut host,
        &trusted(json!({ "id": "api.json#user", "enabled": true })),
    )
    .expect("on");
    assert!(store::read(&mut host, "s1").disabled.is_empty());
}

#[test]
fn an_own_file_name_must_be_plain() {
    let mut host = host();
    for id in ["own:../x.json#a", "own:a/b.json#a", "own:x.txt#a", "nohash"] {
        assert!(
            remove(&mut host, &trusted(json!({ "id": id }))).is_err(),
            "{id}"
        );
    }
}

#[test]
fn untrusted_the_overview_names_the_files_as_blocked_without_reading_them() {
    let mut host = host();
    let overview = overview(&mut host, &json!({ "$network": { "trusted": false } }));
    assert_eq!(
        overview["files"],
        json!([{ "path": ".rbrowser/mocks/api.json", "blocked": true, "rules": [] }])
    );
    assert_eq!(overview["trusted"], false);
}
