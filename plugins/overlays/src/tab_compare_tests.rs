use serde_json::json;

use super::*;

fn order(paths: &[&str]) -> Vec<Stop> {
    paths
        .iter()
        .enumerate()
        .map(|(place, path)| Stop {
            path: (*path).to_owned(),
            selector: format!("sel {path}"),
            index: u32::try_from(place + 1).expect("small"),
        })
        .collect()
}

#[test]
fn the_same_order_everywhere_differs_nowhere() {
    let one = order(&["a", "b", "c"]);
    let two = order(&["a", "b", "c"]);
    let differences = compare(&[(1, one.as_slice()), (2, two.as_slice())]);
    assert_eq!(
        differences,
        json!({ "views": [1, 2], "onlyIn": [], "onlyInCount": 0, "moved": [], "movedCount": 0 })
    );
}

#[test]
fn an_element_some_views_lack_is_listed_with_where_it_is_and_is_not() {
    let mobile = order(&["toggle", "a", "b"]);
    let desktop = order(&["a", "nav", "b"]);
    let differences = compare(&[(1, mobile.as_slice()), (2, desktop.as_slice())]);
    assert_eq!(
        differences["onlyIn"],
        json!([
            { "selector": "sel toggle", "path": "toggle", "views": [1], "missing": [2] },
            { "selector": "sel nav", "path": "nav", "views": [2], "missing": [1] },
        ])
    );
    assert_eq!(differences["moved"], json!([]));
}

#[test]
fn an_element_out_of_place_is_moved_and_the_rest_keep_their_order() {
    // The last element of one view comes first in the other: only it moved.
    let one = order(&["a", "b", "c", "d"]);
    let two = order(&["d", "a", "b", "c"]);
    let differences = compare(&[(1, one.as_slice()), (2, two.as_slice())]);
    assert_eq!(
        differences["moved"],
        json!([{ "selector": "sel d", "path": "d", "positions": [
            { "view": 1, "index": 4 }, { "view": 2, "index": 1 }
        ] }])
    );
    assert_eq!(differences["movedCount"], json!(1));
}

#[test]
fn one_view_has_nothing_to_compare_and_the_lists_are_bounded() {
    let one = order(&["a"]);
    assert_eq!(
        compare(&[(1, one.as_slice())]),
        json!({ "views": [1], "onlyIn": [], "onlyInCount": 0, "moved": [], "movedCount": 0 })
    );
    let many: Vec<String> = (0..60).map(|n| format!("x{n}")).collect();
    let many: Vec<&str> = many.iter().map(String::as_str).collect();
    let full = order(&many);
    let empty = order(&[]);
    let differences = compare(&[(1, full.as_slice()), (2, empty.as_slice())]);
    assert_eq!(
        differences["onlyIn"].as_array().map(Vec::len),
        Some(MAX_DIFFS)
    );
    assert_eq!(differences["onlyInCount"], json!(60));
}

#[test]
fn a_path_twice_in_one_view_counts_once() {
    let one = order(&["a", "a", "b"]);
    let two = order(&["a", "b"]);
    let differences = compare(&[(1, one.as_slice()), (2, two.as_slice())]);
    assert_eq!(differences["onlyInCount"], json!(0));
    assert_eq!(differences["movedCount"], json!(0));
}

#[test]
fn the_differences_of_long_selectors_keep_to_their_byte_budget_with_the_whole_counts() {
    let long = |seed: usize| format!("{seed}{}", "\u{4e2d}".repeat(300));
    let stops = |offset: usize| -> Vec<Stop> {
        (0..60)
            .map(|n| Stop {
                path: long(offset + n),
                selector: long(offset + n),
                index: u32::try_from(n + 1).expect("small"),
            })
            .collect()
    };
    let (one, two) = (stops(0), stops(1000));
    let differences = compare(&[(1, one.as_slice()), (2, two.as_slice())]);
    assert_eq!(differences["onlyInCount"], json!(120));
    let listed = differences["onlyIn"].to_string().len();
    assert!(listed <= MAX_DIFF_BYTES, "{listed} bytes");
    assert!(!differences["onlyIn"].as_array().expect("list").is_empty());
}
