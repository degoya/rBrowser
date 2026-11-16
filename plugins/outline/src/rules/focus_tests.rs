use serde_json::{Value, json};

use crate::rules::test_util::{page, param, short, with};
use crate::rules::*;
use crate::settings::Settings;

/// A focusable `name` at `path` in the box `x, y, width, height`.
fn stop(path: &str, name: &str, rect: [i64; 4]) -> Value {
    json!({
        "selector": path, "path": path, "visible": true, "hidden": false, "box": rect,
        "tag": "a", "role": "link", "name": name
    })
}

fn run(focus: Value) -> Vec<rb_plugin_api::Finding> {
    let outline = page(json!({ "url": "https://site.test/", "focus": focus }));
    findings(
        &[],
        1,
        &outline,
        &["focus".to_owned()],
        &[],
        &Settings::default(),
    )
}

#[test]
fn reading_order_rows_and_columns_are_quiet() {
    // A row of three, the next line, then a second column that starts at the top again.
    let found = run(json!([
        stop("a1", "One", [0, 0, 100, 20]),
        stop("a2", "Two", [110, 0, 100, 20]),
        stop("a3", "Three", [220, 0, 100, 20]),
        stop("a4", "Four", [0, 40, 100, 20]),
        stop("b1", "Column", [400, 0, 100, 20]),
    ]));
    assert!(found.is_empty(), "{:?}", short(&found));
}

#[test]
fn a_jump_up_or_back_along_the_line_is_found_at_the_element_it_lands_on() {
    let found = run(json!([
        stop("a1", "Search", [0, 300, 100, 20]),
        stop("a2", "Logo", [0, 0, 100, 20]),
        stop("a3", "Last", [300, 100, 100, 20]),
        stop("a4", "First", [0, 105, 100, 20]),
    ]));
    assert_eq!(
        short(&found),
        ["focus-order-jump a2", "focus-order-jump a4"]
    );
    assert_eq!(param(&found[0], "from"), Some("link \"Search\""));
    assert_eq!(param(&found[0], "to"), Some("link \"Logo\""));
}

#[test]
fn fixed_hidden_and_unplaced_elements_are_left_out() {
    let found = run(json!([
        stop("a1", "Down", [0, 300, 100, 20]),
        with(
            stop("a2", "Sticky", [0, 0, 100, 20]),
            json!({ "fixed": true })
        ),
        with(
            stop("a3", "Off", [0, 0, 100, 20]),
            json!({ "visible": false })
        ),
        stop("a4", "Nothing", [0, 0, 0, 0]),
        stop("a5", "Below", [0, 400, 100, 20]),
    ]));
    assert!(found.is_empty(), "{:?}", short(&found));
}
