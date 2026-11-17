use serde_json::json;

use super::*;

fn rules() -> Rules {
    Rules::new(
        Box::new(|| {
            vec![
                (ViewId(1), EngineKind::Chromium),
                (ViewId(2), EngineKind::Firefox),
            ]
        }),
        "s1".to_owned(),
    )
}

fn document(clear: u64) -> Value {
    json!({
        "rules": [
            { "id": "mock", "origin": "project", "match": { "urlPattern": "*/api/user" },
              "action": { "type": "mock", "status": 500 } },
            { "id": "block", "origin": "settings", "match": { "urlPattern": "*/tracking.js" },
              "action": { "type": "block" } },
        ],
        "clearSiteData": clear,
    })
}

fn paused(url: &str) -> Paused {
    Paused::of(&json!({ "requestId": "1", "request": { "url": url, "method": "GET" } }))
        .expect("paused")
}

fn ids(rules: &Rules) -> Vec<String> {
    rules
        .active()
        .iter()
        .flat_map(|(_, rules)| rules.iter().map(|rule| rule.id.clone()))
        .collect()
}

#[test]
fn project_rules_apply_only_in_a_trusted_workspace() {
    let rules = rules();
    rules.apply("network-panel", &document(0)).expect("reads");
    assert_eq!(
        ids(&rules),
        ["block"],
        "the person's own block list applies untrusted"
    );
    rules.set_trusted(true);
    assert_eq!(ids(&rules), ["mock", "block"]);
    rules.set_trusted(false);
    assert_eq!(
        ids(&rules),
        ["block"],
        "trust withdrawn: the mocks stop at once"
    );
}

#[test]
fn a_matching_request_counts_a_hit_for_its_rule() {
    let rules = rules();
    rules.set_trusted(true);
    rules.apply("network-panel", &document(0)).expect("reads");
    let Outcome::Call(decision) = rules.decide(&paused("http://a.test/api/user")) else {
        panic!("one call");
    };
    assert_eq!(decision.method, "Fetch.fulfillRequest");
    let _ = rules.decide(&paused("http://a.test/api/user"));
    let _ = rules.decide(&paused("http://a.test/other"));
    let state = rules.state("network-panel");
    assert_eq!(state["hits"], json!({ "mock": 2 }));
    assert_eq!(state["applied"], 2);
    assert_eq!(state["session"], "s1");
    assert_eq!(state["trusted"], true);
}

#[test]
fn the_state_marks_views_whose_engine_is_not_intercepted() {
    let rules = rules();
    rules.set_intercepting(BTreeSet::from([ViewId(1)]));
    assert_eq!(
        rules.state("network-panel")["views"],
        json!([
            { "view": 1, "supported": true, "intercepting": true },
            { "view": 2, "supported": false, "intercepting": false },
        ])
    );
}

#[test]
fn a_raised_clear_counter_asks_for_one_clearing_but_the_first_value_does_not() {
    let rules = rules();
    rules.apply("network-panel", &document(4)).expect("reads");
    assert!(!rules.take_clear(), "the first document only sets the mark");
    rules.apply("network-panel", &document(4)).expect("reads");
    assert!(!rules.take_clear());
    rules.apply("network-panel", &document(5)).expect("reads");
    assert!(rules.take_clear());
    assert!(!rules.take_clear(), "taken once");
}

#[test]
fn an_unreadable_document_drops_the_plugins_rules_and_says_why() {
    let rules = rules();
    rules.set_trusted(true);
    rules.apply("network-panel", &document(0)).expect("reads");
    assert!(rules.apply("network-panel", &json!("nonsense")).is_err());
    assert!(ids(&rules).is_empty());
    assert_eq!(rules.state("network-panel")["rejected"][0]["id"], "*");
}

#[test]
fn rejected_rules_and_the_cache_switch_reach_the_state() {
    let rules = rules();
    let rejected = rules
        .apply(
            "network-panel",
            &json!({ "cacheDisabled": true, "rules": [{ "id": "x", "origin": "project" }] }),
        )
        .expect("reads");
    assert_eq!(rejected.len(), 1);
    assert!(rules.cache_disabled());
    let state = rules.state("network-panel");
    assert_eq!(state["cacheDisabled"], true);
    assert_eq!(
        state["rejected"],
        json!([{ "id": "x", "reason": "no urlPattern" }])
    );
}

#[test]
fn every_change_ticks_the_subscribers() {
    let rules = rules();
    let ticks = rules.subscribe();
    rules.apply("p", &document(0)).expect("reads");
    rules.set_trusted(true);
    rules.set_trusted(true);
    assert_eq!(*ticks.borrow(), 2);
}

#[test]
fn an_unchanged_document_does_not_tick() {
    let rules = rules();
    let ticks = rules.subscribe();
    rules.apply("p", &document(0)).expect("reads");
    rules.apply("p", &document(0)).expect("reads");
    assert_eq!(
        *ticks.borrow(),
        1,
        "the panel's polling sets no session up again"
    );
    assert!(rules.apply("p", &json!(1)).is_err());
    assert!(
        rules.apply("p", &json!(1)).is_err(),
        "the same failure again"
    );
    assert_eq!(*ticks.borrow(), 2);
}

#[test]
fn a_project_plugins_rules_are_all_project_rules_whatever_they_declare() {
    let rules = rules();
    rules
        .apply_from_project("shared", &document(0))
        .expect("reads");
    assert!(
        ids(&rules).is_empty(),
        "a project plugin cannot declare its rules the person's own (review M2, Charge 7)"
    );
    rules.set_trusted(true);
    assert_eq!(ids(&rules), ["mock", "block"]);
    assert!(
        rules
            .active()
            .iter()
            .flat_map(|(_, rules)| rules.iter())
            .all(|rule| rule.origin == Origin::Project)
    );
}

#[test]
fn removing_a_plugin_drops_its_rules_and_its_state() {
    let rules = rules();
    rules.set_trusted(true);
    rules.apply("network-panel", &document(0)).expect("reads");
    rules.apply("other", &document(0)).expect("reads");
    let mut changed = rules.subscribe();
    changed.mark_unchanged();
    rules.remove("network-panel");
    assert!(changed.has_changed().expect("open"), "a removal ticks");
    assert!(
        rules
            .active()
            .iter()
            .all(|(plugin, _)| plugin != "network-panel")
    );
    assert_eq!(ids(&rules), ["mock", "block"], "the other plugin's stay");
    assert_eq!(rules.state("network-panel")["applied"], 0);
    let mut quiet = rules.subscribe();
    quiet.mark_unchanged();
    rules.remove("network-panel");
    assert!(
        !quiet.has_changed().expect("open"),
        "nothing to remove ticks nothing"
    );
}
