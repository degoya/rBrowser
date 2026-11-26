use rb_config::{ProjectConfig, ScopeError, ViewScope};
use rb_engine::{ViewId, ViewInfo};
use rb_mcp::testing::view;
use serde_json::json;

use super::*;

fn config(hidden: &[&str]) -> ProjectConfig {
    serde_json::from_value(json!({
        "version": 1,
        "breakpoints": [
            { "id": "mobile", "width": 393, "height": 852 },
            { "id": "tablet", "width": 768, "height": 1024 },
            { "id": "desktop", "width": 1440, "height": 900 },
            { "id": "fox", "width": 1024, "height": 768, "engine": "firefox" }
        ],
        "layout": { "hidden": hidden }
    }))
    .expect("the test config parses")
}

/// The grid of `config(&["tablet"])`: the hidden breakpoint has no view.
fn grid() -> Vec<ViewInfo> {
    vec![
        view(1, "mobile", "https://a.test/"),
        view(2, "desktop", "https://a.test/"),
        view(3, "fox", "https://a.test/"),
    ]
}

fn ids(views: &[ViewId]) -> Vec<u32> {
    views.iter().map(|id| id.0).collect()
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn all_is_no_narrowing_so_every_view_runs_as_before() {
    let scoped = scoped(&config(&["tablet"]), &grid(), &ViewScope::All).expect("resolves");
    assert_eq!(scoped, None);
}

#[test]
fn visible_takes_the_views_of_the_breakpoints_the_grid_shows() {
    let scoped = scoped(&config(&["tablet"]), &grid(), &ViewScope::Visible)
        .expect("resolves")
        .expect("narrows");
    assert_eq!(ids(&scoped.views), vec![1, 2, 3]);
    assert_eq!(scoped.breakpoints, names(&["mobile", "desktop", "fox"]));
    assert!(scoped.not_shown.is_empty());
}

#[test]
fn chosen_breakpoints_take_only_their_views_in_grid_order() {
    let chosen = ViewScope::Breakpoints(names(&["desktop", "mobile"]));
    let scoped = scoped(&config(&["tablet"]), &grid(), &chosen)
        .expect("resolves")
        .expect("narrows");
    assert_eq!(ids(&scoped.views), vec![1, 2]);
    assert_eq!(scoped.breakpoints, names(&["mobile", "desktop"]));
}

#[test]
fn a_chosen_breakpoint_without_a_view_is_named_not_dropped_silently() {
    let chosen = ViewScope::Breakpoints(names(&["tablet", "mobile"]));
    let scoped = scoped(&config(&["tablet"]), &grid(), &chosen)
        .expect("resolves")
        .expect("narrows");
    assert_eq!(ids(&scoped.views), vec![1]);
    assert_eq!(scoped.not_shown, names(&["tablet"]));
}

#[test]
fn every_view_of_a_breakpoint_runs_its_cell_breakpoint_counting_too() {
    let mut resized = view(4, "mobile", "https://a.test/");
    resized.breakpoint = None;
    let mut views = grid();
    views.push(resized);
    let chosen = ViewScope::Breakpoints(names(&["mobile"]));
    let scoped = scoped(&config(&[]), &views, &chosen)
        .expect("resolves")
        .expect("narrows");
    assert_eq!(ids(&scoped.views), vec![1, 4]);
}

#[test]
fn an_unknown_breakpoint_and_an_empty_choice_are_refused_with_their_code() {
    let unknown = ViewScope::Breakpoints(names(&["watch"]));
    let refused = scoped(&config(&[]), &grid(), &unknown).expect_err("unknown");
    assert_eq!(
        refused,
        RunScopeError::Scope(ScopeError::UnknownBreakpoint("watch".to_owned()))
    );
    assert_eq!(refused.code(), "scope.error.unknown_breakpoint");
    let empty = ViewScope::Breakpoints(Vec::new());
    let refused = scoped(&config(&[]), &grid(), &empty).expect_err("empty");
    assert_eq!(refused.code(), "scope.error.empty");
}

#[test]
fn a_choice_no_view_shows_is_refused_naming_the_breakpoints() {
    let chosen = ViewScope::Breakpoints(names(&["tablet"]));
    let refused = scoped(&config(&["tablet"]), &grid(), &chosen).expect_err("no view");
    assert_eq!(refused, RunScopeError::NoView(names(&["tablet"])));
    assert_eq!(refused.code(), NO_VIEW_KEY);
    assert!(refused.to_string().contains("tablet"));
}

#[test]
fn the_scope_argument_is_taken_out_of_a_commands_arguments() {
    let (rest, scope) =
        take_scope(&json!({ "scope": "visible", "url": "https://a.test/" })).expect("reads");
    assert_eq!(scope, Some(ViewScope::Visible));
    assert_eq!(rest, json!({ "url": "https://a.test/" }));
    let (rest, scope) = take_scope(&json!({ "scope": ["mobile"] })).expect("reads");
    assert_eq!(scope, Some(ViewScope::Breakpoints(names(&["mobile"]))));
    assert_eq!(rest, json!({}));
    let (rest, scope) = take_scope(&serde_json::Value::Null).expect("none");
    assert_eq!((rest, scope), (serde_json::Value::Null, None));
    let (rest, scope) = take_scope(&json!({ "scope": null, "target": 2 })).expect("null");
    assert_eq!((rest, scope), (json!({ "target": 2 }), None));
    assert!(take_scope(&json!({ "scope": "hidden" })).is_err());
    assert!(take_scope(&json!({ "scope": 3 })).is_err());
}

#[test]
fn a_plugin_calls_scope_becomes_its_breakpoints() {
    let config = config(&["tablet"]);
    let fed = plugin_args(
        &config,
        &json!({ "$scope": "visible", "url": "https://a.test/" }),
    )
    .expect("resolves");
    assert_eq!(
        fed,
        json!({
            "$scope": "visible",
            "breakpoints": ["mobile", "desktop", "fox"],
            "url": "https://a.test/"
        })
    );
    let fed = plugin_args(&config, &json!({ "$scope": "all" })).expect("all");
    assert_eq!(fed, json!({ "$scope": "all" }));
    let same = json!({ "breakpoints": ["mobile"] });
    assert_eq!(plugin_args(&config, &same).expect("untouched"), same);
    let refused = plugin_args(&config, &json!({ "$scope": ["watch"] })).expect_err("unknown");
    assert_eq!(refused.code(), "scope.error.unknown_breakpoint");
    let both = json!({ "$scope": "visible", "breakpoints": ["mobile"] });
    assert_eq!(
        plugin_args(&config, &both).expect_err("both").code(),
        BOTH_KEY
    );
}

#[test]
fn the_breakpoints_no_view_shows_are_named_in_a_warning() {
    assert_eq!(not_shown_notice(&[]), None);
    let Some(CoreToShell::Notification(notice)) = not_shown_notice(&names(&["tablet", "watch"]))
    else {
        panic!("a notice");
    };
    assert_eq!(notice.level, NotificationLevel::Warning);
    assert_eq!(notice.message_key, NOT_SHOWN_KEY);
    assert_eq!(notice.args["breakpoints"], "tablet, watch");
}

#[test]
fn a_plugin_command_of_the_visible_views_gets_its_scope_unless_the_call_names_one() {
    let visible = Some(&ViewScope::Visible);
    assert_eq!(
        with_default_scope(&json!({}), visible),
        json!({ "$scope": "visible" })
    );
    let named = json!({ "$scope": ["mobile"] });
    assert_eq!(with_default_scope(&named, visible), named);
    let own = json!({ "breakpoints": ["mobile"] });
    assert_eq!(with_default_scope(&own, visible), own);
    assert_eq!(with_default_scope(&json!({}), None), json!({}));
}
