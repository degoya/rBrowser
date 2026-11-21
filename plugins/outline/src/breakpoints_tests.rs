use rb_plugin_api::codes;
use serde_json::json;

use super::*;
use crate::fake::FakeHost;

const URL: &str = "https://site.test/";

/// A link `name` at `path`, shown or hidden, its place `order` and its box `rect`.
fn link(path: &str, name: &str, visible: bool, order: i64, rect: [i64; 4]) -> Value {
    json!({
        "selector": path, "path": path, "visible": visible, "hidden": !visible,
        "tag": "a", "role": "link", "name": name, "order": order, "box": rect
    })
}

fn heading(path: &str, text: &str, visible: bool) -> Value {
    json!({ "selector": path, "path": path, "visible": visible, "hidden": !visible, "level": 2, "text": text })
}

/// Three breakpoints of one page: the menu link "Pricing" only on the desktop (the tablet lacks
/// it, the phone hides it), "Home" everywhere; the phone shows the buttons of a row reordered.
fn host() -> FakeHost {
    let home = link("header > a", "Home", true, 1, [0, 0, 80, 30]);
    let mut host = FakeHost::default();
    host.view(1, "Desktop", URL)
        .view(2, "Tablet", URL)
        .view(3, "Phone", URL)
        .page(
            1,
            &json!({ "url": URL, "headings": [heading("h2", "Offers", true)], "focus": [
                home.clone(), link("nav > a", "Pricing", true, 2, [96, 0, 80, 30])
            ] }),
        )
        .page(
            2,
            &json!({ "url": URL, "headings": [heading("h2", "Offers", true)], "focus": [home.clone()] }),
        )
        .page(
            3,
            &json!({ "url": URL, "dir": "ltr", "headings": [heading("h2", "Offers", false)], "focus": [
                link("header > a", "Home", true, 1, [96, 0, 80, 30]),
                link("nav > a", "Pricing", false, 2, [0, 0, 0, 0]),
                link("main > a", "Docs", true, 3, [0, 0, 80, 30])
            ] }),
        );
    host
}

#[test]
fn the_diff_is_one_row_per_element_with_the_breakpoints_missing_it() {
    let mut host = host();
    let answer = breakpoint_diff(&mut host, &json!({})).expect("answer");
    let rows = answer["rows"].as_array().expect("rows");
    let pricing: Vec<&Value> = rows
        .iter()
        .filter(|r| r["label"] == "link \"Pricing\"")
        .collect();
    assert_eq!(pricing.len(), 1, "{rows:?}");
    assert_eq!(pricing[0]["kind"], "interactive");
    assert_eq!(pricing[0]["shown"], json!([1]));
    assert_eq!(pricing[0]["missing"], json!([2, 3]));
    assert_eq!(pricing[0]["url"], URL);
    assert_eq!(pricing[0]["views"]["3"]["visible"], false);
    let offers = rows
        .iter()
        .find(|r| r["kind"] == "headings")
        .expect("the heading");
    assert_eq!(offers["missing"], json!([3]));
    let labels: Vec<&str> = answer["views"]
        .as_array()
        .expect("views")
        .iter()
        .filter_map(|v| v["label"].as_str())
        .collect();
    assert_eq!(labels, ["Desktop", "Tablet", "Phone"]);
    let pricing_findings: Vec<&Value> = answer["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter(|f| f["rule"] == "interactive-differs" && f["params"]["text"] == "link \"Pricing\"")
        .collect();
    assert_eq!(
        pricing_findings.len(),
        2,
        "one per view lacking it, one element"
    );
    assert!(
        pricing_findings
            .iter()
            .all(|f| f["path"] == "nav > a" && f["params"]["missing"] == "Tablet, Phone")
    );
}

#[test]
fn the_diff_takes_its_elements_from_the_call_or_the_settings() {
    let mut host = host();
    let answer = breakpoint_diff(&mut host, &json!({ "elements": ["headings"] })).expect("answer");
    let kinds: Vec<&str> = answer["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .filter_map(|r| r["kind"].as_str())
        .collect();
    assert_eq!(kinds, ["headings"]);
    host.settings
        .insert("diff.elements".to_owned(), "[]".to_owned());
    let none = breakpoint_diff(&mut host, &json!({})).expect("answer");
    assert_eq!(none["rows"], json!([]));
    assert_eq!(none["elements"], json!([]));
    for bad in [
        json!({ "elements": "headings" }),
        json!({ "elements": ["links"] }),
    ] {
        let failure = breakpoint_diff(&mut host, &bad).expect_err("refused");
        assert_eq!(failure.code(), Some(codes::INVALID));
    }
}

#[test]
fn one_view_is_compared_with_the_others_on_its_page() {
    let mut host = host();
    let answer = breakpoint_diff(&mut host, &json!({ "target": 2 })).expect("answer");
    let rows = answer["rows"].as_array().expect("rows");
    assert!(
        rows.iter().all(|r| r["missing"]
            .as_array()
            .is_some_and(|missing| missing.contains(&json!(2)))),
        "only what the tablet lacks: {rows:?}"
    );
    assert!(
        answer["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .all(|f| f["view"] == 2)
    );
    assert!(!rows.is_empty());
}

#[test]
fn the_reading_order_names_both_elements_per_view() {
    let mut host = host();
    let answer = reading_order(&mut host, &json!({})).expect("answer");
    let findings = answer["findings"].as_array().expect("findings");
    let reading: Vec<&Value> = findings
        .iter()
        .filter(|f| f["rule"] == "reading-order")
        .collect();
    assert_eq!(reading.len(), 1, "{findings:?}");
    assert_eq!(reading[0]["view"], 3);
    assert_eq!(reading[0]["params"]["from"], "link \"Home\"");
    assert_eq!(reading[0]["params"]["to"], "link \"Docs\"");
    assert_eq!(reading[0]["data"]["other"]["selector"], "header > a");
    assert!(
        findings
            .iter()
            .all(|f| f["rule"] == "reading-order" || f["rule"] == "focus-order-jump")
    );
    let one = reading_order(&mut host, &json!({ "target": 1 })).expect("answer");
    assert_eq!(one["findings"], json!([]));
    assert_eq!(one["views"][0]["dir"], "ltr");
}

#[test]
fn the_person_s_rule_levels_hold_in_the_answers() {
    let mut host = host();
    host.settings.insert(
        rb_plugin_api::RULE_LEVELS_SETTING.to_owned(),
        json!({ "reading-order": "off", "interactive-differs": "warning" }).to_string(),
    );
    let order = reading_order(&mut host, &json!({})).expect("answer");
    assert!(
        order["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .all(|f| f["rule"] != "reading-order")
    );
    let diff = breakpoint_diff(&mut host, &json!({})).expect("answer");
    assert!(
        diff["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .filter(|f| f["rule"] == "interactive-differs")
            .all(|f| f["severity"] == "warning")
    );
}

#[test]
fn the_commands_show_their_tab_of_the_panel() {
    for (command, tab) in [
        ("breakpoint_diff", "breakpoints"),
        ("reading_order", "order"),
    ] {
        let answer = show(command);
        assert_eq!(answer["$open"], json!({ "panel": "outline" }));
        assert_eq!(
            answer["$post"],
            json!([{ "target": "panel", "message": { "type": "show", "tab": tab } }])
        );
    }
}
