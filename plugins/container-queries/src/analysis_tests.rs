use rb_plugin_api::ViewInfo;
use serde_json::{Value, json};

use super::*;

fn view(id: u32, width: u32) -> ViewInfo {
    ViewInfo {
        id,
        label: format!("{width}px"),
        breakpoint: format!("bp{width}"),
        engine: "chromium".to_owned(),
        width,
        ..ViewInfo::default()
    }
}

/// A view's report of the fixture with 3 containers and 4 @container rules
/// (tests/fixtures/containers.html), the card `card_width` wide.
pub(crate) fn fixture_report(card_width: f64) -> Value {
    let card_matching = if card_width >= 500.0 {
        json!([0])
    } else if card_width <= 300.0 {
        json!([1])
    } else {
        json!([])
    };
    json!({
        "containers": [
            { "path": "html > body:nth-child(2) > main:nth-child(1)", "selector": "main.shell", "names": ["shell"],
              "type": "inline-size", "inline": 1000, "block": 600, "conditions": [2], "matching": [],
              "thresholds": { "inline": [400, 900], "block": [] }, "resized": false },
            { "path": "#hero", "selector": "article#hero", "names": ["card"], "type": "inline-size",
              "inline": card_width, "block": 300, "conditions": [0, 1], "matching": card_matching,
              "thresholds": { "inline": [300, 500], "block": [] }, "resized": card_width < 600.0 },
            { "path": "html > body:nth-child(2) > main:nth-child(1) > aside:nth-child(3)", "selector": "aside.side",
              "names": [], "type": "size", "inline": 300, "block": 250, "conditions": [3], "matching": [3],
              "thresholds": { "inline": [], "block": [200] }, "resized": false }
        ],
        "conditions": [
            { "name": "card", "query": "(min-width: 500px)", "supported": true,
              "bounds": [{ "axis": "inline", "op": ">=", "value": "500px", "px": 500 }] },
            { "name": "card", "query": "(max-width: 300px)", "supported": true,
              "bounds": [{ "axis": "inline", "op": "<=", "value": "300px", "px": 300 }] },
            { "name": "shell", "query": "(400px <= width < 900px)", "supported": true,
              "bounds": [{ "axis": "inline", "op": ">=", "value": "400px", "px": 400 },
                         { "axis": "inline", "op": "<", "value": "900px", "px": 900 }] },
            { "name": "", "query": "(min-height: 200px)", "supported": true,
              "bounds": [{ "axis": "block", "op": ">=", "value": "200px", "px": 200 }] }
        ],
        "unreadableSheets": 0,
        "truncated": false
    })
}

fn parse(value: &Value) -> ViewReport {
    ViewReport::parse(&value.to_string()).expect("a report")
}

fn merged(views: Vec<(ViewInfo, Result<ViewReport, String>)>) -> Value {
    serde_json::to_value(merge(views)).expect("JSON")
}

#[test]
fn the_fixture_lists_exactly_4_conditions_and_3_containers_with_their_sizes() {
    let analysis = merged(vec![(view(1, 1280), Ok(parse(&fixture_report(640.0))))]);
    let conditions = analysis["conditions"].as_array().expect("conditions");
    assert_eq!(conditions.len(), 4);
    assert_eq!(conditions[2]["query"], json!("(400px <= width < 900px)"));
    assert_eq!(conditions[2]["bounds"][1]["px"], json!(900.0));
    let containers = analysis["containers"].as_array().expect("containers");
    let sizes: Vec<(Value, Value, Value)> = containers
        .iter()
        .map(|c| {
            (
                c["selector"].clone(),
                c["views"][0]["inline"]["size"].clone(),
                c["views"][0]["block"]["size"].clone(),
            )
        })
        .collect();
    assert_eq!(
        sizes,
        vec![
            (json!("main.shell"), json!(1000.0), json!(600.0)),
            (json!("article#hero"), json!(640.0), json!(300.0)),
            (json!("aside.side"), json!(300.0), json!(250.0)),
        ]
    );
}

#[test]
fn each_container_names_its_next_bound_below_and_above() {
    let analysis = merged(vec![(view(1, 1280), Ok(parse(&fixture_report(640.0))))]);
    let card = &analysis["containers"][1]["views"][0];
    assert_eq!(
        card["inline"],
        json!({ "size": 640.0, "below": 500.0, "above": null })
    );
    assert_eq!(card["matching"], json!([0]));
    let shell = &analysis["containers"][0]["views"][0];
    assert_eq!(
        shell["inline"],
        json!({ "size": 1000.0, "below": 900.0, "above": null })
    );
    let side = &analysis["containers"][2]["views"][0];
    assert_eq!(
        side["block"],
        json!({ "size": 250.0, "below": 200.0, "above": null })
    );
    assert_eq!(
        side["inline"],
        json!({ "size": 300.0, "below": null, "above": null })
    );
}

#[test]
fn a_container_dragged_to_400_px_no_longer_matches_min_width_500() {
    let analysis = merged(vec![(view(1, 1280), Ok(parse(&fixture_report(400.0))))]);
    let card = &analysis["containers"][1]["views"][0];
    assert_eq!(card["matching"], json!([]));
    assert_eq!(card["resized"], json!(true));
    assert_eq!(
        card["inline"],
        json!({ "size": 400.0, "below": 300.0, "above": 500.0 })
    );
}

#[test]
fn the_views_are_ordered_by_width_and_each_switch_names_what_changed() {
    let analysis = merged(vec![
        (view(2, 1280), Ok(parse(&fixture_report(640.0)))),
        (view(1, 375), Ok(parse(&fixture_report(280.0)))),
        (view(3, 768), Ok(parse(&fixture_report(450.0)))),
    ]);
    assert_eq!(analysis["conditions"].as_array().map(Vec::len), Some(4));
    let views: Vec<Value> = analysis["views"]
        .as_array()
        .expect("views")
        .iter()
        .map(|v| v["view"].clone())
        .collect();
    assert_eq!(views, vec![json!(1), json!(3), json!(2)]);
    let card = &analysis["containers"][1];
    assert_eq!(card["conditions"], json!([0, 1]));
    assert_eq!(
        card["switches"],
        json!([
            { "from": 1, "to": 3, "gained": [], "lost": [1] },
            { "from": 3, "to": 2, "gained": [0], "lost": [] }
        ])
    );
    assert_eq!(analysis["containers"][0]["switches"], json!([]));
}

#[test]
fn a_view_that_failed_is_named_with_its_code_and_adds_nothing() {
    let analysis = merged(vec![
        (view(1, 375), Err("plugins.error.bad_output".to_owned())),
        (view(2, 1280), Ok(parse(&fixture_report(640.0)))),
    ]);
    assert_eq!(
        analysis["views"][0]["error"],
        json!("plugins.error.bad_output")
    );
    assert_eq!(analysis["views"][1]["containers"], json!(3));
    assert!(analysis["views"][1].get("error").is_none());
    assert_eq!(
        analysis["containers"][1]["views"].as_array().map(Vec::len),
        Some(1)
    );
}

#[test]
fn a_report_is_bounded_and_its_indices_checked() {
    let mut report = fixture_report(640.0);
    report["containers"][1]["conditions"] = json!([0, 1, 99]);
    report["containers"][1]["matching"] = json!([7]);
    report["containers"][0]["selector"] = json!("x".repeat(1000));
    let parsed = parse(&report);
    let card = &parsed.containers[1];
    assert_eq!(card.conditions, vec![0, 1]);
    assert!(card.matching.is_empty());
    assert_eq!(parsed.containers[0].selector.chars().count(), MAX_TEXT);
    let many: Vec<Value> = (0..MAX_CONTAINERS + 5)
        .map(|i| json!({ "path": format!("#c{i}") }))
        .collect();
    let cut = parse(&json!({ "containers": many }));
    assert_eq!(cut.containers.len(), MAX_CONTAINERS);
    assert!(cut.truncated);
    assert!(ViewReport::parse("not json").is_none());
    assert!(ViewReport::parse("[1, 2]").is_none());
}

#[test]
fn next_bounds_take_the_nearest_thresholds_on_either_side() {
    assert_eq!(
        next_bounds(640.0, &[300.0, 500.0, 900.0]),
        (Some(500.0), Some(900.0))
    );
    assert_eq!(next_bounds(500.0, &[300.0, 500.0]), (Some(500.0), None));
    assert_eq!(next_bounds(10.0, &[]), (None, None));
}
