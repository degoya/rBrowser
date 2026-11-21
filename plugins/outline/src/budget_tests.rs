use serde_json::json;

use super::*;

#[test]
fn an_answer_that_fits_is_untouched() {
    let mut answer = json!({ "findings": [1, 2], "sections": { "links": { "rows": [1] } } });
    let before = answer.clone();
    fit(&mut answer, 1000);
    assert_eq!(answer, before);
}

#[test]
fn the_longest_list_is_halved_until_it_fits_and_marked() {
    let rows: Vec<String> = (0..400).map(|n| format!("row number {n}")).collect();
    let mut answer = json!({
        "findings": ["a"],
        "sections": { "links": { "rows": rows } }
    });
    fit(&mut answer, 2_000);
    assert!(serde_json::to_string(&answer).expect("json").len() <= 2_000);
    assert_eq!(answer["sections"]["links"]["cut"], true);
    assert_eq!(answer["findings"], json!(["a"]));
    assert!(answer.get("cut").is_none());
}

#[test]
fn trees_and_per_view_lists_are_cut_too() {
    let nodes: Vec<u32> = (0..2000).collect();
    let mut answer = json!({
        "sections": { "axtree": { "views": { "1": { "nodes": nodes } } } },
        "views": [{ "outline": { "links": nodes } }]
    });
    fit(&mut answer, 3_000);
    assert!(serde_json::to_string(&answer).expect("json").len() <= 3_000);
    assert_eq!(answer["sections"]["axtree"]["views"]["1"]["cut"], true);
    assert_eq!(answer["views"][0]["outline"]["cut"], true);
}

#[test]
fn findings_are_cut_at_the_top() {
    let findings: Vec<u32> = (0..1000).collect();
    let mut answer = json!({ "findings": findings });
    fit(&mut answer, 500);
    assert_eq!(answer["cut"], true);
}

#[test]
fn an_answer_without_lists_stays_as_it_is() {
    let mut answer = json!({ "views": "x".repeat(100) });
    fit(&mut answer, 10);
    assert_eq!(answer["views"].as_str().map(str::len), Some(100));
}

#[test]
fn the_rows_of_the_breakpoint_diff_are_cut_too() {
    let rows: Vec<String> = (0..400).map(|n| format!("difference {n}")).collect();
    let mut answer = json!({ "views": [], "rows": rows, "findings": [] });
    fit(&mut answer, 2_000);
    assert!(serde_json::to_string(&answer).expect("json").len() <= 2_000);
    assert_eq!(answer["cut"], true);
}
