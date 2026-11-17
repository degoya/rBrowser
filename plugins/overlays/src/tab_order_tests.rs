use rb_plugin_api::feature::Feature;
use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, Failure, ViewInfo};
use serde_json::{Value, json};

use super::*;
use crate::Overlays;
use crate::state::{STATE_FILE, STATE_PATH};

fn view(id: u32) -> ViewInfo {
    ViewInfo {
        id,
        label: format!("view {id}"),
        breakpoint: format!("bp{id}"),
        engine: "chromium".to_owned(),
        width: 375,
        height: 800,
        dpr: 1.0,
        url: "http://localhost/tab-order/".to_owned(),
        title: "Tab order".to_owned(),
    }
}

fn host() -> MemoryHost {
    let mut host = MemoryHost::new()
        .grant(Capability::Views)
        .grant(Capability::Evaluate)
        .grant(Capability::Findings)
        .grant(Capability::ProjectRead)
        .grant(Capability::OutWrite);
    host.views = vec![view(1), view(2)];
    host.evaluate_answer = LIST.to_owned();
    host
}

fn call(host: &mut MemoryHost, tool: &str, args: Value) -> Result<Value, Failure> {
    let answer = Overlays.call_tool(host, tool, &args.to_string());
    if let Some(bytes) = host.out.get(STATE_FILE).cloned() {
        host.project.insert(STATE_PATH.to_owned(), bytes);
    }
    answer.map(|text| serde_json::from_str(&text).expect("JSON"))
}

/// What `__rbOverlaysMeasure.tabOrder` answers for a page with a positive tabindex first and a
/// closed menu link.
const LIST: &str = r##"{"count":3,"items":[
  {"index":1,"selector":"#promo","path":"html > body > a","tabindex":3,"rect":{"x":0,"y":40,"width":60,"height":20},"warnings":["positive-tabindex"]},
  {"index":2,"selector":"nav a","path":"html > body > nav > a","tabindex":0,"rect":{"x":-240,"y":0,"width":60,"height":20},"warnings":["invisible-offscreen"]},
  {"index":3,"selector":"x-card >>> button","path":"html > body > x-card >>> button","tabindex":0,"rect":{"x":0,"y":90,"width":60,"height":20},"warnings":["reverse-jump"]}
]}"##;

#[test]
fn the_tool_answers_each_views_ordered_list_and_the_differences() {
    let mut host = host();
    let answer = call(&mut host, "tab_order", json!({})).expect("tab order");
    let views = answer["views"].as_array().expect("views");
    assert_eq!(views.len(), 2);
    assert_eq!(views[0]["view"], json!(1));
    assert_eq!(views[0]["count"], json!(3));
    let first = &views[0]["items"][0];
    assert_eq!(first["index"], json!(1));
    assert_eq!(first["selector"], json!("#promo"));
    assert_eq!(
        first["rect"],
        json!({ "x": 0.0, "y": 40.0, "width": 60.0, "height": 20.0 })
    );
    assert_eq!(first["warnings"], json!(["positive-tabindex"]));
    assert_eq!(answer["differences"]["onlyIn"], json!([]));
    assert_eq!(answer["differences"]["moved"], json!([]));
    let (_, script) = host.evaluated.last().expect("evaluated");
    assert!(
        script.contains(r#"tabOrder({"includeIframes":true})"#),
        "{script}"
    );
    // A query reports no findings; switching the overlay on does.
    assert!(host.findings.is_empty());
}

#[test]
fn the_iframes_come_from_the_option_else_the_setting() {
    let mut host = host();
    host.settings
        .insert("tabOrder.includeIframes".to_owned(), "false".to_owned());
    call(&mut host, "tab_order", json!({ "target": 2 })).expect("setting");
    let (view, script) = host.evaluated.last().expect("evaluated");
    assert_eq!(*view, 2);
    assert!(script.contains(r#"{"includeIframes":false}"#), "{script}");
    call(&mut host, "tab_order", json!({ "includeIframes": true })).expect("option");
    let (_, script) = host.evaluated.last().expect("evaluated");
    assert!(script.contains(r#"{"includeIframes":true}"#), "{script}");
    let refused = call(&mut host, "tab_order", json!({ "includeIframes": "yes" }));
    assert!(refused.is_err());
}

#[test]
fn a_view_that_cannot_be_measured_answers_its_error_and_stays_out_of_the_comparison() {
    let mut host = host();
    host.evaluate_answer = "\"not a list\"".to_owned();
    let answer = call(&mut host, "tab_order", json!({})).expect("tab order");
    assert_eq!(
        answer["views"][0]["error"],
        json!("plugins.error.bad_output")
    );
    assert_eq!(answer["differences"]["views"], json!([]));
}

#[test]
fn a_list_from_the_page_is_bounded_and_cleaned() {
    let long = "x".repeat(400);
    let items: Vec<Value> = (0..MAX_ITEMS + 5)
        .map(|n| {
            json!({ "index": n + 1, "selector": format!("a\u{7}{long}"), "path": "p",
                    "warnings": ["reverse-jump", "<script>"], "rect": { "x": 1.5 } })
        })
        .collect();
    let text = json!({ "count": MAX_ITEMS + 5, "items": items }).to_string();
    let list = TabList::parse(&text).expect("list");
    assert_eq!(list.count, u64::try_from(MAX_ITEMS + 5).expect("small"));
    assert_eq!(list.items.len(), MAX_ITEMS);
    assert_eq!(list.items[0].selector.chars().count(), MAX_TEXT_CHARS);
    assert!(!list.items[0].selector.contains('\u{7}'));
    assert_eq!(list.items[0].warnings, vec!["reverse-jump".to_owned()]);
    assert!(TabList::parse("[1, 2]").is_none());
    assert!(TabList::parse(&json!(LIST).to_string()).is_some());
}

#[test]
fn switching_the_overlay_on_reports_the_warnings_as_findings_and_posts_the_differences() {
    let mut host = host();
    let answer = call(&mut host, "set", json!({ "overlay": "tab-order" })).expect("on");
    assert_eq!(answer["overlays"]["tab-order"]["enabled"], json!(true));
    assert_eq!(answer["tabOrder"]["views"][1]["count"], json!(3));
    let rules: Vec<(&str, Option<u32>, &str)> = host
        .findings
        .iter()
        .map(|finding| {
            let label = finding
                .params
                .iter()
                .find(|(name, _)| name == "label")
                .map_or("", |(_, value)| value.as_str());
            (finding.rule.as_str(), finding.view, label)
        })
        .collect();
    assert_eq!(
        rules,
        vec![
            ("tab-positive-tabindex", Some(1), "3"),
            ("tab-invisible-focusable", Some(1), "offscreen"),
            ("tab-positive-tabindex", Some(2), "3"),
            ("tab-invisible-focusable", Some(2), "offscreen"),
        ]
    );
    assert_eq!(
        host.findings[1].message,
        "plugin.overlays.findings.tab_invisible_focusable"
    );
    assert_eq!(
        host.findings[1].path.as_deref(),
        Some("html > body > nav > a")
    );
    let posts = answer["$post"].as_array().expect("posts");
    let panel = posts
        .iter()
        .find(|post| post["message"]["type"] == json!("tab-order"))
        .expect("differences posted");
    assert_eq!(panel["target"], json!("panel"));
    assert_eq!(panel["message"]["differences"]["views"], json!([1, 2]));

    host.findings.clear();
    let off = call(
        &mut host,
        "set",
        json!({ "overlay": "tab-order", "enabled": false }),
    )
    .expect("off");
    assert!(off.get("tabOrder").is_none());
    assert!(host.findings.is_empty());
}

#[test]
fn a_focus_in_one_view_is_posted_to_every_view_while_live() {
    let mut host = host();
    let message =
        json!({ "view": 2, "message": { "type": "tab-focus", "path": "html > body > a" } });
    let answer = call(&mut host, "@page", message.clone()).expect("focus");
    assert_eq!(
        answer["$post"],
        json!([{ "target": "all", "message": { "type": "tab-focus", "view": 2, "path": "html > body > a" } }])
    );
    host.settings
        .insert("tabOrder.live".to_owned(), "false".to_owned());
    assert_eq!(call(&mut host, "@page", message).expect("quiet"), json!({}));
    for path in [
        json!(""),
        json!(7),
        json!("a\u{0}b"),
        json!("x".repeat(301)),
    ] {
        let bad = json!({ "view": 2, "message": { "type": "tab-focus", "path": path } });
        assert!(call(&mut host, "@page", bad).is_err());
    }
}

#[test]
fn the_command_toggles_the_overlay() {
    let mut host = host();
    let on = Overlays
        .run_command(&mut host, "tab_order", "{}")
        .expect("on");
    let on: Value = serde_json::from_str(&on).expect("JSON");
    assert_eq!(on["overlays"]["tab-order"]["enabled"], json!(true));
}
