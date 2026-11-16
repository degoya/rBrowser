use std::collections::BTreeMap;

use rb_plugin_types::{RuleInfo, Severity};
use serde_json::json;

use super::*;

fn rule(id: &str, title: &str) -> RuleInfo {
    RuleInfo {
        id: id.to_owned(),
        title: title.to_owned(),
        severity: Severity::Warning,
        shared_id: None,
        help_url: None,
    }
}

/// RB-080-72: a rule override is chosen by name; the id it stores is the qualified one.
#[test]
fn a_rule_is_offered_by_its_title_under_its_qualified_id() {
    let texts = BTreeMap::from([(
        "seo.rule.images_alt".to_owned(),
        "Images have alt text".to_owned(),
    )]);
    let options = options_of(
        "seo",
        "SEO",
        &[
            rule("images.alt", "seo.rule.images_alt"),
            rule("title.length", "seo.rule.untranslated"),
        ],
        &texts,
    );
    assert_eq!(
        options,
        [
            RuleOption {
                value: "seo.images.alt".to_owned(),
                label: "SEO - Images have alt text".to_owned(),
            },
            RuleOption {
                value: "seo.title.length".to_owned(),
                label: "SEO - title.length".to_owned(),
            },
        ]
    );
    assert_eq!(
        data_of(&options[..1], false),
        json!({ "options": [{ "value": "seo.images.alt", "label": "SEO - Images have alt text" }], "open": true })
    );
    assert_eq!(data_of(&[], true)["loading"], json!(true));
}

fn key(project: &str, trusted: bool, plugins: &[&str]) -> CatalogKey {
    CatalogKey {
        project: Some(std::path::PathBuf::from(project)),
        trusted,
        plugins: plugins.iter().map(|id| (*id).to_owned()).collect(),
    }
}

fn option(value: &str) -> RuleOption {
    RuleOption {
        value: value.to_owned(),
        label: value.to_owned(),
    }
}

/// Review 5: project B never shows project A's rules; trust and the plugin set count too.
#[test]
fn a_list_holds_only_for_its_project_trust_and_plugins() {
    let a = key("/work/a", true, &["seo"]);
    let state = State {
        done: Some((a.clone(), vec![option("seo.images.alt")])),
        running: None,
    };
    assert_eq!(
        next(&state, &a),
        Next::Ready(vec![option("seo.images.alt")])
    );
    assert_eq!(next(&state, &key("/work/b", true, &["seo"])), Next::Start);
    assert_eq!(next(&state, &key("/work/a", false, &["seo"])), Next::Start);
    assert_eq!(
        next(&state, &key("/work/a", true, &["a11y-axe", "seo"])),
        Next::Start
    );
}

/// Review 3: a second request while the list is enumerated waits, it does not start again.
#[test]
fn a_running_enumeration_is_waited_for() {
    let b = key("/work/b", true, &["seo"]);
    let state = State {
        done: None,
        running: Some(b.clone()),
    };
    assert_eq!(next(&state, &b), Next::Wait);
    assert_eq!(next(&state, &key("/work/c", true, &["seo"])), Next::Start);
}

/// Review 4: the running mark goes with the enumeration, however it ends.
#[test]
fn the_running_mark_is_cleared_when_the_enumeration_ends_or_panics() {
    let k = key("/work/guard-test", true, &["x"]);
    let running = Running::start(&mut lock(&STATE), k.clone());
    assert_eq!(lock(&STATE).running.as_ref(), Some(&k));
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _running = running;
        panic!("a plugin's describe failed hard");
    }));
    assert!(outcome.is_err());
    assert_ne!(lock(&STATE).running.as_ref(), Some(&k));
}
