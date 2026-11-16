use serde_json::json;

use super::*;
use crate::fake_host::FakeHost;

const MOCKS: &str = r#"{ "rules": [
  { "id": "user", "match": { "urlPattern": "*/api/user" }, "mock": { "status": 500, "delayMs": 800 } },
  { "id": "off", "enabled": false, "match": { "urlPattern": "*/x" }, "block": true },
  { "id": "tracking", "match": { "urlPattern": "*/tracking.js" }, "block": true }
] }"#;

fn host() -> FakeHost {
    let mut host = FakeHost::default();
    host.file(".rbrowser/mocks/api.json", MOCKS);
    host.set("block", r#"["*/ads/*"]"#);
    host
}

fn context(trusted: bool) -> Context {
    Context::read(&json!({ "$network": { "session": "s1", "trusted": trusted } }))
}

fn ids(document: &Value) -> Vec<String> {
    document["rules"]
        .as_array()
        .map(|rules| {
            rules
                .iter()
                .filter_map(|r| r["id"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_trusted_project_sends_its_enabled_mocks_and_the_block_list() {
    let mut host = host();
    let document = document(&mut host, &context(true));
    assert_eq!(
        ids(&document),
        ["api.json#user", "api.json#tracking", "settings:block-0"]
    );
    assert_eq!(document["rules"][2]["origin"], "settings");
    assert_eq!(document["cacheDisabled"], false);
    assert_eq!(document["clearSiteData"], 0);
}

#[test]
fn an_untrusted_project_sends_no_mock() {
    let mut host = host();
    host.file(".rbrowser/mocks/bodies/x.json", "secret");
    let document = document(&mut host, &context(false));
    assert_eq!(ids(&document), ["settings:block-0"]);
}

#[test]
fn mocks_switched_off_for_the_session_or_one_rule_switched_off_stay_out() {
    let mut host = host();
    let mut state = store::read(&mut host, "s1");
    state.disabled.insert("api.json#tracking".to_owned());
    store::write(&mut host, &state).expect("written");
    assert_eq!(
        ids(&document(&mut host, &context(true))),
        ["api.json#user", "settings:block-0"]
    );
    state.mocks_enabled = Some(false);
    state.cache_disabled = Some(true);
    store::write(&mut host, &state).expect("written");
    let off = document(&mut host, &context(true));
    assert_eq!(ids(&off), ["settings:block-0"]);
    assert_eq!(off["cacheDisabled"], true);
}

#[test]
fn the_document_carries_breakpoints_settings_and_the_asks_until_answered() {
    let mut host = host();
    host.set("breakpoints.timeoutMs", "5000");
    let mut state = store::read(&mut host, "s1");
    state.breakpoints.push(
        json!({ "id": "bp-1", "urlPattern": "*/api/*", "regex": false,
        "method": "GET", "stage": "response", "views": [1], "enabled": true }),
    );
    state.breakpoints_enabled = Some(false);
    state.asks = vec![
        json!({ "id": "a1", "kind": "abort", "held": "h1", "allViews": false }),
        json!({ "id": "a2", "kind": "abort", "held": "h2", "allViews": false }),
        json!({ "id": "a3", "kind": "abort", "held": "h3", "allViews": false }),
    ];
    store::write(&mut host, &state).expect("written");
    let answered = Context::read(&json!({ "$network": { "session": "s1", "trusted": false,
        "answers": { "a1": { "done": true, "resolved": 1 }, "a2": { "error": "unknown_held" } } } }));
    let document = document(&mut host, &answered);
    assert_eq!(
        document["breakpoints"],
        json!([{ "id": "bp-1", "match": { "urlPattern": "*/api/*", "regex": false, "method": "GET" },
                 "stage": "response", "enabled": true, "views": [1] }])
    );
    assert_eq!(document["breakpointsEnabled"], false);
    assert_eq!(document["settings"]["breakpointTimeoutMs"], 5000);
    assert_eq!(document["settings"]["allowComposeFromMcp"], false);
    assert_eq!(
        document["asks"],
        json!([{ "id": "a3", "kind": "abort", "held": "h3", "allViews": false }])
    );
    let kept = store::read(&mut host, "s1");
    assert_eq!(kept.asks.len(), 1);
    assert_eq!(kept.last_error.as_deref(), Some("unknown_held"));
}
