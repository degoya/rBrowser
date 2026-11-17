use serde_json::json;

use super::*;
use crate::testing::FakeHost;

/// What `src/measure.js` answers for the fixture paragraph: `count` lines of `chars` characters
/// and a short last line.
fn paragraph(count: usize, chars: u64) -> Value {
    let mut lines: Vec<Value> = (0..count)
        .map(|at| json!({ "chars": chars, "x": 24, "y": 200 + at * 27, "width": 1200, "height": 27 }))
        .collect();
    lines.push(json!({ "chars": 12, "x": 24, "y": 200 + count * 27, "width": 90, "height": 27 }));
    json!({ "elements": [{ "path": "p#measure", "lines": lines }], "truncated": false })
}

fn host() -> FakeHost {
    let mut host = FakeHost::default();
    host.view(1, "chromium")
        .view(2, "chromium")
        .view(3, "firefox");
    host.views[0].label = "Mobile".into();
    host.views[1].label = "Desktop".into();
    host.evaluate.insert(1, paragraph(16, 38).to_string());
    host.evaluate.insert(2, paragraph(4, 151).to_string());
    host
}

#[test]
fn the_fixture_paragraph_is_long_on_desktop_and_short_on_mobile() {
    let mut host = host();
    let answer = tool(&mut host, &json!({ "target": "all" })).expect("measured");
    let views = answer["views"].as_array().expect("views");
    let mobile = &views[0];
    assert_eq!(mobile["average"], 38.0);
    assert_eq!(mobile["rating"], "short");
    assert_eq!(mobile["over"], 0);
    let desktop = &views[1];
    assert_eq!(desktop["average"], 151.0);
    assert_eq!(desktop["max"], 151);
    assert_eq!(desktop["lines"], 5);
    assert_eq!(desktop["over"], 4);
    assert_eq!(desktop["rating"], "long");
    assert_eq!(desktop["items"][0]["path"], "p#measure");
    assert_eq!(
        views[2]["error"],
        "plugin.color-picker.errors.unsupported_engine"
    );
    // The desktop view's long lines are marked there; the panel gets the result.
    let posts = answer["$post"].as_array().expect("posts");
    assert_eq!(posts[0]["target"], 1);
    assert_eq!(posts[0]["message"]["marks"], json!([]));
    assert_eq!(posts[1]["target"], 2);
    assert_eq!(
        posts[1]["message"]["marks"].as_array().map(Vec::len),
        Some(4)
    );
    assert_eq!(posts[1]["message"]["marks"][0]["chars"], 151);
    assert_eq!(posts[2]["target"], "panel");
    assert_eq!(posts[2]["message"]["result"]["selector"], "p, li");
    // The setting's selector reached the collector.
    assert!(host.evaluated[0].1.contains("\"selector\":\"p, li\""));
}

#[test]
fn the_average_leaves_out_last_lines_and_keeps_single_ones() {
    let stats = Stats::of(&[vec![60, 70, 10], vec![50]], 80);
    assert_eq!(stats.lines, 4);
    assert_eq!(stats.average, 60.0);
    assert_eq!(stats.max, 70);
    assert_eq!(stats.over, 0);
    let settings = Settings::default();
    assert_eq!(stats.rating(&settings), "optimal");
    assert_eq!(Stats::of(&[], 80).rating(&settings), "none");
}

#[test]
fn the_command_measures_every_view_and_shows_the_panel() {
    let mut host = host();
    let answer = command(&mut host, &json!({})).expect("measured");
    assert_eq!(answer["$open"]["panel"], "picker");
    assert_eq!(answer["views"].as_array().map(Vec::len), Some(3));
}

#[test]
fn choose_and_clear_only_post() {
    let mut host = host();
    let choose = tool(&mut host, &json!({ "choose": true })).expect("choose");
    assert_eq!(choose["$post"][0]["message"]["type"], "choose");
    assert_eq!(choose["$post"][0]["message"]["label"], "Colour picker");
    let clear = tool(&mut host, &json!({ "clear": true })).expect("clear");
    assert_eq!(clear["$post"][0]["message"]["type"], "clear");
    assert!(host.evaluated.is_empty());
    let odd = tool(&mut host, &json!({ "clear": "yes" })).expect_err("odd");
    assert_eq!(odd.code.as_deref(), Some("plugins.error.invalid"));
}

#[test]
fn a_selector_is_checked_here_and_by_the_page() {
    let mut host = host();
    for selector in [json!(""), json!("p".repeat(1025)), json!(5)] {
        let refused = tool(&mut host, &json!({ "selector": selector })).expect_err("refused");
        assert_eq!(refused.code.as_deref(), Some("plugins.error.invalid"));
    }
    host.evaluate
        .insert(1, json!({ "invalidSelector": true }).to_string());
    let answer = tool(&mut host, &json!({ "target": 1, "selector": "p[" })).expect("answer");
    assert_eq!(
        answer["views"][0]["error"],
        "plugin.color-picker.errors.bad_selector"
    );
}
