use serde_json::json;

use super::*;
use crate::testing::FakeHost;

/// What `src/palette.js` answers for `tests/fixtures/site/colors/index.html` (its Vitest checks
/// the counts against the page): five colours, each used a different number of times.
pub(crate) fn fixture_answer() -> Value {
    json!({
        "elements": 30,
        "truncated": false,
        "entries": [
            { "value": "rgb(253, 216, 53)", "background": 2 },
            { "value": "rgb(30, 136, 229)", "background": 5 },
            { "value": "rgb(33, 33, 33)", "color": 7 },
            { "value": "rgb(67, 160, 71)", "background": 3 },
            { "value": "rgb(229, 57, 53)", "background": 2, "border": 2 },
        ],
    })
}

#[test]
fn the_fixture_palette_names_its_five_colours_by_use() {
    let mut host = FakeHost::default();
    host.view(1, "chromium");
    host.evaluate.insert(1, fixture_answer().to_string());
    let answer = tool(&mut host, &json!({})).expect("palette");
    let colors = answer["views"][0]["colors"].as_array().expect("colours");
    let hexes: Vec<&str> = colors.iter().filter_map(|c| c["hex"].as_str()).collect();
    assert_eq!(
        hexes,
        ["#212121", "#1e88e5", "#e53935", "#43a047", "#fdd835"]
    );
    assert_eq!(
        colors[2]["uses"],
        json!({ "color": 0, "background": 2, "border": 2 })
    );
    assert_eq!(colors[0]["count"], 7);
    assert_eq!(answer["views"][0]["elements"], 30);
    // The collector ran in the active view's plugin world.
    assert!(host.evaluated[0].1.contains("maxElements"));
}

#[test]
fn near_colours_group_under_the_most_used() {
    let groups = group(&[
        json!({ "value": "rgb(30, 136, 229)", "background": 1 }),
        json!({ "value": "rgb(31, 136, 229)", "background": 4 }),
        json!({ "value": "rgb(0, 0, 0)", "color": 2 }),
        json!({ "value": "rgba(31, 136, 229, 0.5)", "background": 1 }),
    ]);
    assert_eq!(groups.len(), 3);
    assert_eq!(groups[0].color.hex(), "#1f88e5");
    assert_eq!(groups[0].count(), 5);
    assert_eq!(
        groups[0].members,
        ["rgb(31, 136, 229)", "rgb(30, 136, 229)"]
    );
    // A translucent version is a colour of its own.
    assert_eq!(groups[2].color.hex(), "#1f88e580");
}

#[test]
fn transparent_unused_and_unreadable_entries_are_left_out() {
    let groups = group(&[
        json!({ "value": "rgba(0, 0, 0, 0)", "background": 9 }),
        json!({ "value": "rgb(1, 2, 3)" }),
        json!({ "value": "color(display-p3 1 0 0)", "color": 3 }),
        json!({ "value": 5, "color": 3 }),
    ]);
    assert!(groups.is_empty());
}

#[test]
fn a_mark_posts_the_members_to_every_view() {
    let mut host = FakeHost::default();
    let answer = tool(
        &mut host,
        &json!({ "mark": ["rgb(30, 136, 229)", "rgb(31, 136, 229)"] }),
    )
    .expect("mark");
    assert_eq!(answer["marked"], 2);
    assert_eq!(answer["$post"][0]["target"], "all");
    assert_eq!(answer["$post"][0]["message"]["type"], "mark");
    let clear = tool(&mut host, &json!({ "mark": [] })).expect("clear");
    assert_eq!(clear["$post"][0]["message"]["type"], "unmark");
    for mark in [json!("red"), json!(["no colour"]), json!(vec!["#fff"; 17])] {
        let refused = tool(&mut host, &json!({ "mark": mark })).expect_err("refused");
        assert_eq!(refused.code.as_deref(), Some("plugins.error.invalid"));
    }
    assert!(host.evaluated.is_empty());
}
