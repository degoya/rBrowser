use rb_config::ProjectConfig;
use rb_engine::{ViewId, ViewInfo};
use rb_mcp::testing::view;
use serde_json::json;

use super::*;

fn config() -> ProjectConfig {
    serde_json::from_value(json!({
        "version": 1,
        "breakpoints": [
            { "id": "mobile", "width": 393, "height": 852 },
            { "id": "tablet", "width": 768, "height": 1024 },
            { "id": "desktop", "width": 1440, "height": 900 }
        ],
        "layout": { "hidden": ["tablet"] }
    }))
    .expect("the test config parses")
}

fn grid() -> Vec<ViewInfo> {
    vec![
        view(1, "mobile", "https://a.test/"),
        view(2, "desktop", "https://a.test/"),
    ]
}

#[test]
fn neither_views_nor_a_scope_leaves_the_flow_its_own() {
    assert_eq!(
        views_of(&config(), &grid(), &json!({ "name": "login" })).expect("none"),
        (None, Vec::new())
    );
    assert_eq!(
        views_of(&config(), &grid(), &json!({ "scope": "all" })).expect("all"),
        (None, Vec::new())
    );
}

#[test]
fn the_scope_takes_the_views_of_its_breakpoints() {
    let (views, not_shown) =
        views_of(&config(), &grid(), &json!({ "scope": "visible" })).expect("visible");
    assert_eq!(views, Some(vec![ViewId(1), ViewId(2)]));
    assert!(not_shown.is_empty());
    let (views, not_shown) = views_of(
        &config(),
        &grid(),
        &json!({ "scope": ["desktop", "tablet"] }),
    )
    .expect("chosen");
    assert_eq!(views, Some(vec![ViewId(2)]));
    assert_eq!(not_shown, ["tablet"]);
}

#[test]
fn views_named_win_over_a_scope() {
    let (views, _) = views_of(
        &config(),
        &grid(),
        &json!({ "views": [1], "scope": ["desktop"] }),
    )
    .expect("views");
    assert_eq!(views, Some(vec![ViewId(1)]));
}

#[test]
fn an_unknown_breakpoint_or_a_choice_no_view_shows_is_refused_with_its_code() {
    for (scope, code) in [
        (json!(["watch"]), "scope.error.unknown_breakpoint"),
        (json!(["tablet"]), "scope.error.no_view"),
        (json!([]), "scope.error.empty"),
    ] {
        let refused =
            views_of(&config(), &grid(), &json!({ "scope": scope })).expect_err("refused");
        assert_eq!(refused.code(), "flows.no_views");
        assert!(refused.to_string().contains(code), "{refused}");
    }
}
