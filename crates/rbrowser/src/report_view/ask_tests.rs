use serde_json::json;

use super::*;

#[test]
fn an_answer_without_the_key_is_left_alone() {
    let answer = json!({ "runs": [] });
    let (rest, ask) = split(answer.clone());
    assert_eq!(rest, answer);
    assert_eq!(ask, None);
    let (rest, ask) = split(json!("text"));
    assert_eq!(rest, json!("text"));
    assert_eq!(ask, None);
}

#[test]
fn the_key_is_taken_out_of_the_answer() {
    let (rest, ask) = split(json!({
        "opened": { "run": "20261112-100000" },
        "$report": { "open": "20261112-100000/mobile.html", "window": "perReport" },
    }));
    assert_eq!(rest, json!({ "opened": { "run": "20261112-100000" } }));
    assert_eq!(
        ask,
        Some(Ask {
            open: Some("20261112-100000/mobile.html".to_owned()),
            window: Placement::PerReport,
            remove: Vec::new(),
        })
    );
}

#[test]
fn an_unknown_window_is_the_one_report_window() {
    let (_, ask) = split(json!({ "$report": { "open": "a/b.html", "window": "grid" } }));
    assert_eq!(ask.map(|ask| ask.window), Some(Placement::Single));
    let (_, ask) = split(json!({ "$report": { "open": "a/b.html" } }));
    assert_eq!(ask.map(|ask| ask.window), Some(Placement::Single));
}

#[test]
fn removals_are_names_and_bounded() {
    let many: Vec<String> = (0..100).map(|n| format!("run-{n}")).collect();
    let (_, ask) = split(json!({ "$report": { "remove": many } }));
    let ask = ask.expect("an ask");
    assert_eq!(ask.remove.len(), MAX_REMOVE);
    assert_eq!(ask.open, None);
    let (_, ask) = split(json!({ "$report": { "remove": ["a", 3, null, "b"] } }));
    assert_eq!(ask.expect("an ask").remove, vec!["a", "b"]);
}

#[test]
fn a_malformed_ask_asks_for_nothing() {
    for bad in [
        json!({ "$report": "open" }),
        json!({ "$report": { "open": 3 } }),
        json!({ "$report": {} }),
        json!({ "$report": { "remove": "a" } }),
    ] {
        let (rest, ask) = split(bad);
        assert_eq!(ask, None);
        assert!(
            rest.get(REPORT_KEY).is_none(),
            "the key never reaches the caller"
        );
    }
}

#[test]
fn only_a_bundled_plugin_of_the_list_may_ask() {
    assert!(may_ask("lighthouse", PluginSource::Bundled));
    assert!(!may_ask("lighthouse", PluginSource::Project));
    assert!(!may_ask("lighthouse", PluginSource::User));
    assert!(!may_ask("coverage", PluginSource::Bundled));
}
