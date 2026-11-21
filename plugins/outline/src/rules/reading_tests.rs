use serde_json::{Value, json};

use super::*;
use crate::rules::test_util::{at, heading, page, param, short, with};
use crate::rules::*;

/// A button `name` at `id`, its place in the document `order`, its box `rect`.
fn button(id: &str, name: &str, order: i64, rect: [i64; 4]) -> Value {
    with(
        at(id),
        json!({ "tag": "button", "role": "button", "name": name, "order": order, "box": rect }),
    )
}

fn outline(focus: Vec<Value>, headings: Vec<Value>, dir: &str) -> crate::page::PageOutline {
    page(json!({ "url": "https://site.test/", "dir": dir, "focus": focus, "headings": headings }))
}

fn run(outline: &crate::page::PageOutline, tolerance: i64) -> Vec<rb_plugin_api::Finding> {
    let mut sink = Sink::new(&[]);
    check(&mut sink, ("https://site.test/", 1), outline, tolerance);
    sink.findings
}

/// Three buttons in a row, 80 px wide with 16 px between them, at the x of each.
fn row(xs: [i64; 3]) -> Vec<Value> {
    ["#a", "#b", "#c"]
        .into_iter()
        .zip(["A", "B", "C"])
        .zip(xs)
        .enumerate()
        .map(|(at, ((id, name), x))| {
            let order = i64::try_from(at).expect("small");
            button(id, name, order + 1, [x, 0, 80, 30])
        })
        .collect()
}

#[test]
fn order_minus_one_on_the_second_of_three_buttons_is_one_finding_with_both_elements() {
    // The acceptance fixture: `order: -1` puts B first; the DOM keeps A, B, C.
    let moved = outline(row([96, 0, 192]), Vec::new(), "ltr");
    let found = run(&moved, 4);
    assert_eq!(short(&found), ["reading-order #b"]);
    assert_eq!(found[0].severity, rb_plugin_api::Severity::Warning);
    assert_eq!(param(&found[0], "from"), Some("button \"A\""));
    assert_eq!(param(&found[0], "to"), Some("button \"B\""));
    let data: Value = serde_json::from_str(found[0].data.as_deref().expect("data")).expect("json");
    assert_eq!(data["other"]["selector"], "#a");
    assert_eq!(data["other"]["path"], "#a");

    let natural = outline(row([0, 96, 192]), Vec::new(), "ltr");
    assert!(run(&natural, 4).is_empty(), "without order no finding");
}

#[test]
fn right_to_left_content_reads_from_the_right() {
    let natural = outline(row([192, 96, 0]), Vec::new(), "rtl");
    assert!(run(&natural, 4).is_empty());
    let left_to_right = outline(row([0, 96, 192]), Vec::new(), "rtl");
    assert_eq!(
        short(&run(&left_to_right, 4)),
        ["reading-order #b", "reading-order #c"]
    );
}

#[test]
fn a_column_to_the_right_is_read_after_the_left_one_and_a_left_one_first() {
    let main = button("#main-end", "End", 1, [0, 400, 600, 30]);
    let aside_right = button("#aside", "Aside", 2, [640, 0, 200, 30]);
    assert!(
        run(
            &outline(vec![main.clone(), aside_right], Vec::new(), "ltr"),
            4
        )
        .is_empty()
    );
    let aside_left = button("#aside", "Aside", 2, [0, 0, 200, 30]);
    let main_right = button("#main-end", "End", 1, [240, 400, 600, 30]);
    assert_eq!(
        short(&run(
            &outline(vec![main_right, aside_left], Vec::new(), "ltr"),
            4
        )),
        ["reading-order #aside"]
    );
}

#[test]
fn headings_and_interactive_elements_read_in_one_sequence_in_document_order() {
    // flex-direction: column-reverse: the heading comes first in the source, last on screen.
    let title = with(
        heading("#title", 2, "Offers"),
        json!({ "order": 1, "box": [0, 100, 300, 40] }),
    );
    let buy = button("#buy", "Buy", 2, [0, 0, 80, 30]);
    let found = run(&outline(vec![buy], vec![title], "ltr"), 4);
    assert_eq!(short(&found), ["reading-order #buy"]);
    assert_eq!(param(&found[0], "from"), Some("heading \"Offers\""));
}

#[test]
fn hidden_fixed_and_boxless_elements_have_no_place_in_the_reading_order() {
    let mut buttons = row([96, 0, 192]);
    for (at, change) in [
        json!({ "hidden": true, "visible": false }),
        json!({ "fixed": true }),
        json!({ "box": [0, 0, 0, 0] }),
    ]
    .into_iter()
    .enumerate()
    {
        buttons[1] = with(buttons[1].clone(), change);
        assert!(
            run(&outline(buttons.clone(), Vec::new(), "ltr"), 4).is_empty(),
            "case {at}"
        );
        buttons = row([96, 0, 192]);
    }
}

#[test]
fn the_tolerance_lets_neighbours_overlap_a_little() {
    // B ends 3 px inside A's box.
    let overlapping = outline(
        vec![
            button("#a", "A", 1, [96, 0, 80, 30]),
            button("#b", "B", 2, [0, 0, 99, 30]),
        ],
        Vec::new(),
        "ltr",
    );
    assert_eq!(short(&run(&overlapping, 4)), ["reading-order #b"]);
    assert!(run(&overlapping, 0).is_empty());
    let same_box = outline(
        vec![
            button("#a", "A", 1, [10, 10, 10, 10]),
            button("#b", "B", 2, [10, 10, 10, 10]),
        ],
        Vec::new(),
        "ltr",
    );
    assert!(run(&same_box, crate::settings::MAX_TOLERANCE_PX).is_empty());
}

#[test]
fn the_rule_is_part_of_the_findings_of_a_view_with_headings_and_focus() {
    let moved = outline(row([96, 0, 192]), Vec::new(), "ltr");
    let settings = crate::settings::Settings::default();
    let all = findings(&[READING_ORDER.to_owned()], 1, &moved, &[], &[], &settings);
    assert_eq!(short(&all), ["reading-order #b"]);
    let focus_only = findings(&[], 1, &moved, &["focus".to_owned()], &[], &settings);
    assert!(
        focus_only.iter().all(|f| f.rule != READING_ORDER),
        "the reading order needs the headings too"
    );
}

/// The shared cases of the reading order (`tests/fixtures/reading-order/cases.json`): the tab
/// order overlay's reading arrows (plugins/overlays) decide them alike (RB-094-02).
#[test]
fn the_shared_reading_order_cases_are_decided_alike() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/reading-order/cases.json"
    ))
    .expect("JSON");
    for case in fixture["cases"].as_array().expect("cases") {
        let name = case["name"].as_str().expect("name");
        let focus = case["boxes"]
            .as_array()
            .expect("boxes")
            .iter()
            .enumerate()
            .map(|(at, rect)| {
                let order = i64::try_from(at).expect("small");
                let rect: [i64; 4] = serde_json::from_value(rect.clone()).expect("box");
                button(&format!("#e{at}"), &format!("E{at}"), order + 1, rect)
            })
            .collect();
        let dir = case["dir"].as_str().expect("dir");
        let tolerance = case["tolerance"].as_i64().expect("tolerance");
        let found = run(&outline(focus, Vec::new(), dir), tolerance);
        let expected: Vec<String> = case["outOfPlace"]
            .as_array()
            .expect("places")
            .iter()
            .map(|at| format!("reading-order #e{}", at.as_u64().expect("place")))
            .collect();
        assert_eq!(short(&found), expected, "{name}");
    }
}
