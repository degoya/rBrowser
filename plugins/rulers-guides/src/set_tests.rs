use super::*;
use crate::store::{STATE_FILE, STATE_PATH};
use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, ViewInfo};
use serde_json::{Value, json};

fn view(id: u32, breakpoint: &str) -> ViewInfo {
    ViewInfo {
        id,
        label: breakpoint.to_owned(),
        breakpoint: breakpoint.to_owned(),
        engine: "chromium".to_owned(),
        width: 375,
        height: 800,
        dpr: 1.0,
        url: "https://example.test/".to_owned(),
        title: String::new(),
    }
}

fn host() -> MemoryHost {
    let mut host = MemoryHost::new()
        .grant(Capability::Views)
        .grant(Capability::ProjectRead)
        .grant(Capability::OutWrite);
    host.views = vec![view(1, "mobile"), view(2, "desktop")];
    host
}

fn set(host: &mut MemoryHost, args: Value) -> Result<Value, Failure> {
    run(host, &args.to_string()).map(|text| serde_json::from_str(&text).expect("JSON answer"))
}

fn persist(host: MemoryHost) -> MemoryHost {
    let bytes = host
        .out
        .get(STATE_FILE)
        .cloned()
        .expect("state.json written");
    host.with_file(STATE_PATH, &bytes)
}

#[test]
fn no_arguments_answer_the_state_and_change_nothing() {
    let mut host = host();
    let answer = set(&mut host, json!({})).expect("state");
    assert_eq!(answer["breakpoints"], json!(["desktop", "mobile"]));
    assert_eq!(answer["state"]["guides"], json!([]));
    // The panel's placeholders (RB-080-66).
    assert_eq!(answer["templates"]["bootstrap"]["columns"], 12);
    assert!(answer.get("$post").is_none());
    assert!(answer.get("saved").is_none());
    assert!(host.out.is_empty());
}

#[test]
fn guides_and_columns_per_breakpoint_are_set_and_answered() {
    let mut host = host();
    let answer = set(
        &mut host,
        json!({
            "guides": [{"axis": "x", "unit": "percent", "value": 50}, {"axis": "x", "value": 320}],
            "columns": {
                "mobile": {"columns": 4, "gutter": 16, "margin": 16},
                "desktop": {"columns": 12, "gutter": 24, "margin": 12, "maxWidth": 1200}
            },
            "visible": {"columns": true},
        }),
    )
    .expect("set");
    assert_eq!(answer["saved"], true);
    assert_eq!(
        answer["state"]["guides"],
        json!([
            {"id": 1, "axis": "x", "unit": "percent", "value": 50.0},
            {"id": 2, "axis": "x", "unit": "px", "value": 320.0}
        ])
    );
    assert_eq!(answer["state"]["columns"]["desktop"]["maxWidth"], 1200.0);
    assert_eq!(
        answer["state"]["visible"],
        json!({"rulers": false, "columns": true, "baseline": false})
    );
    assert_eq!(answer["$post"][0]["target"], "all");
    // Kept: the next call reads them back.
    let mut later = persist(host);
    let again = set(&mut later, json!({})).expect("state");
    assert_eq!(again["state"], answer["state"]);
}

#[test]
fn one_setup_with_a_view_target_is_that_views_breakpoint() {
    let mut host = host();
    let answer = set(
        &mut host,
        json!({"columns": {"preset": "bootstrap"}, "target": 1}),
    )
    .expect("set");
    assert_eq!(answer["breakpoint"], "mobile");
    assert_eq!(
        answer["state"]["columns"],
        json!({"*": {"preset": "bootstrap"}, "mobile": {"preset": "bootstrap"}})
    );
    let mut host = self::host();
    let all = set(
        &mut host,
        json!({"columns": {"preset": "tailwind"}, "target": "all"}),
    )
    .expect("set");
    assert_eq!(
        all["state"]["columns"],
        json!({"*": {"preset": "tailwind"}})
    );
}

#[test]
fn null_switches_a_shared_setup_off_and_reset_brings_it_back() {
    let mut host = host();
    host.settings.insert(
        "columns".to_owned(),
        r#"{"mobile":{"columns":4}}"#.to_owned(),
    );
    let off = set(&mut host, json!({"columns": {"mobile": null}})).expect("off");
    assert_eq!(off["state"]["columns"], json!({}));
    let mut host = persist(host);
    let back = set(&mut host, json!({"reset": true})).expect("reset");
    assert_eq!(back["state"]["columns"], json!({"mobile": {"columns": 4}}));
}

#[test]
fn visible_true_shows_the_rulers_and_the_baseline_is_set() {
    let mut host = host();
    let answer = set(
        &mut host,
        json!({"visible": true, "baseline": {"lineHeight": 24, "offset": 4}}),
    )
    .expect("set");
    assert_eq!(answer["state"]["visible"]["rulers"], true);
    assert_eq!(
        answer["state"]["baseline"],
        json!({"lineHeight": 24.0, "offset": 4.0})
    );
}

#[test]
fn out_of_range_and_unknown_arguments_are_refused() {
    let mut host = host();
    let reason = |result: Result<Value, Failure>| {
        result
            .expect_err("refused")
            .params
            .get("reason")
            .cloned()
            .unwrap_or_default()
    };
    assert_eq!(
        reason(set(
            &mut host,
            json!({"guides": [{"axis": "x", "unit": "percent", "value": 101}]})
        )),
        "guide.value"
    );
    assert_eq!(
        reason(set(
            &mut host,
            json!({"guides": [{"axis": "y", "unit": "percent", "value": 1}]})
        )),
        "guide.unit"
    );
    assert_eq!(
        reason(set(&mut host, json!({"columns": {"columns": 0}}))),
        "columns.columns"
    );
    assert_eq!(
        reason(set(&mut host, json!({"columns": {"x y": {"columns": 2}}}))),
        "columns.breakpoint"
    );
    assert_eq!(reason(set(&mut host, json!({"columns": [1]}))), "columns");
    assert_eq!(
        reason(set(&mut host, json!({"baseline": {"lineHeight": 0.5}}))),
        "baseline.lineHeight"
    );
    assert_eq!(
        reason(set(&mut host, json!({"target": "mobile"}))),
        "target"
    );
    assert_eq!(
        reason(set(&mut host, json!({"script": "alert(1)"}))),
        "arguments"
    );
    assert!(host.out.is_empty());
}

#[test]
fn colours_are_set_per_element_and_reset_brings_the_shared_ones_back() {
    let mut host = host();
    host.settings
        .insert("colors.guide".to_owned(), "\"#123456\"".to_owned());
    let answer = set(
        &mut host,
        json!({"colors": {"ruler": "#000000cc", "baseline": "#ff000080"}}),
    )
    .expect("set");
    assert_eq!(answer["state"]["colors"]["ruler"], "#000000cc");
    assert_eq!(answer["state"]["colors"]["baseline"], "#ff000080");
    // The shared setting stays where nothing overrides it.
    assert_eq!(answer["state"]["colors"]["guide"], "#123456");
    // Every view follows at once: the state goes to all of them.
    assert_eq!(answer["$post"][0]["target"], "all");
    assert_eq!(
        answer["$post"][0]["message"]["overrides"]["colors"],
        json!({"ruler": "#000000cc", "baseline": "#ff000080"})
    );
    let mut host = persist(host);
    let again = set(&mut host, json!({"colors": {"guide": "#00ff00"}})).expect("set");
    assert_eq!(again["state"]["colors"]["ruler"], "#000000cc");
    assert_eq!(again["state"]["colors"]["guide"], "#00ff00");
    let mut host = persist(host);
    let reset = set(&mut host, json!({"reset": true})).expect("reset");
    assert_eq!(reset["state"]["colors"]["ruler"], "#f6f7f9f0");
    assert_eq!(reset["state"]["colors"]["guide"], "#123456");
}

#[test]
fn a_colour_that_is_none_or_unknown_is_refused() {
    let mut host = host();
    for args in [
        json!({"colors": {"guide": "red"}}),
        json!({"colors": {"guide": "#00ff00;}"}}),
        json!({"colors": {"text": "#000000"}}),
        json!({"colors": "#000000"}),
    ] {
        assert!(set(&mut host, args.clone()).is_err(), "{args}");
    }
    assert!(host.out.is_empty());
}
