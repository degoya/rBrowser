use super::*;
use serde_json::json;

fn issues_of(value: Value) -> Vec<ConfigIssue> {
    match validate(value) {
        Err(ConfigError::Invalid(issues)) => issues,
        other => panic!("expected issues, got {other:?}"),
    }
}

fn base() -> Value {
    json!({
        "version": 1,
        "breakpoints": [
            { "id": "mobile", "width": 393, "height": 852 },
            { "id": "desktop", "width": 1440, "height": 900 }
        ],
        "layout": { "columns": 2, "rows": 1, "cells": [
            { "breakpoint": "mobile", "col": 1, "row": 1 },
            { "breakpoint": "desktop", "col": 2, "row": 1 }
        ] }
    })
}

#[test]
fn a_consistent_config_passes() {
    let config = validate(base()).expect("valid");
    assert_eq!(config.breakpoints.len(), 2);
}

#[test]
fn a_duplicate_breakpoint_id_is_reported_at_the_second_id() {
    let mut value = base();
    value["breakpoints"][1]["id"] = json!("mobile");
    value["layout"]["cells"][1]["breakpoint"] = json!("mobile");
    let issues = issues_of(value);
    assert_eq!(issues.len(), 1, "{issues:?}");
    assert_eq!(issues[0].path, "/breakpoints/1/id");
    assert_eq!(issues[0].key, "config.error.duplicate_id");
}

#[test]
fn an_unknown_cell_breakpoint_is_reported_at_the_cell() {
    let mut value = base();
    value["layout"]["cells"][1]["breakpoint"] = json!("tablet");
    let issues = issues_of(value);
    assert_eq!(issues.len(), 1, "{issues:?}");
    assert_eq!(issues[0].path, "/layout/cells/1/breakpoint");
    assert_eq!(issues[0].key, "config.error.unknown_breakpoint");
}

#[test]
fn a_cell_outside_the_grid_is_reported_at_the_cell() {
    let mut value = base();
    value["layout"]["cells"][1]["colSpan"] = json!(2);
    let issues = issues_of(value);
    assert_eq!(issues.len(), 1, "{issues:?}");
    assert_eq!(issues[0].path, "/layout/cells/1");
    assert_eq!(issues[0].key, "config.error.cell_outside_grid");

    let mut value = base();
    value["layout"]["cells"][0]["row"] = json!(0);
    assert_eq!(issues_of(value)[0].path, "/layout/cells/0");
}

#[test]
fn all_rule_problems_are_reported_together() {
    let mut value = base();
    value["breakpoints"][1]["id"] = json!("mobile");
    value["layout"]["cells"][1] = json!({ "breakpoint": "mobile", "col": 3, "row": 1 });
    value["layout"]["cells"][0]["zoom"] = json!(2.0);
    value["layouts"] =
        json!({ "default": "nope", "presets": { "f": { "mode": "focus", "focus": "x" } } });
    let keys: Vec<String> = issues_of(value).into_iter().map(|i| i.key).collect();
    assert_eq!(
        keys,
        [
            "config.error.duplicate_id",
            "config.error.zoom_range",
            "config.error.cell_outside_grid",
            "config.error.unknown_preset",
            "config.error.unknown_breakpoint",
        ]
    );
}

#[test]
fn a_preset_layout_is_checked_like_the_main_one() {
    let mut value = base();
    value["layouts"] = json!({ "presets": { "a/b": { "layout": { "columns": 1, "rows": 1,
        "cells": [{ "breakpoint": "mobile", "col": 2, "row": 1 }] } } } });
    let issues = issues_of(value);
    assert_eq!(issues[0].path, "/layouts/presets/a~1b/layout/cells/0");
}

#[test]
fn sessions_must_be_listed_except_default() {
    let mut value = base();
    value["breakpoints"][0]["session"] = json!("default");
    value["breakpoints"][1]["session"] = json!("admin");
    let issues = issues_of(value.clone());
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].path, "/breakpoints/1/session");
    value["sessions"] = json!(["admin"]);
    validate(value).expect("listed session is valid");
}

#[test]
fn a_type_error_names_its_json_pointer() {
    let mut value = base();
    value["layout"]["cells"][1]["col"] = json!("two");
    let issues = issues_of(value);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].path, "/layout/cells/1/col");
    assert_eq!(issues[0].key, "config.error.invalid_value");

    let mut value = base();
    value["sync"] = json!({ "scroll": "yes" });
    assert_eq!(issues_of(value)[0].path, "/sync/scroll");
}

#[test]
fn a_missing_required_field_names_its_object() {
    let mut value = base();
    value["breakpoints"][0]
        .as_object_mut()
        .expect("object")
        .remove("id");
    let issues = issues_of(value);
    assert_eq!(issues[0].path, "/breakpoints/0");
    assert!(issues[0].message.contains("id"), "{}", issues[0].message);
}

/// The start URL and the environments' URLs open in every view: only `http`/`https`, so an
/// untrusted repository cannot point the grid at `file:`, `data:` or `javascript:` (charge-4
/// review, finding 2). The check refuses them for the settings window and `config_set` alike.
#[test]
fn the_start_url_and_the_environment_urls_must_be_http_or_https() {
    let mut value = base();
    value["startUrl"] = json!("file:///C:/Users/me/");
    value["environments"] = json!([
        { "id": "local", "url": "http://localhost:3000" },
        { "id": "phish", "url": "data:text/html,<h1>login</h1>" },
        { "id": "script", "url": "javascript:alert(1)" }
    ]);
    let issues = issues_of(value);
    let keys: Vec<_> = issues
        .iter()
        .map(|issue| (issue.path.as_str(), issue.key.as_str()))
        .collect();
    assert_eq!(
        keys,
        [
            ("/startUrl", START_URL_KEY),
            ("/environments/1/url", ENVIRONMENT_URL_KEY),
            ("/environments/2/url", ENVIRONMENT_URL_KEY),
        ]
    );
    let mut value = base();
    value["startUrl"] = json!("https://example.com/app?x=1");
    value["environments"] = json!([{ "id": "local", "url": "HTTP://127.0.0.1:8080/" }]);
    validate(value).expect("http and https pass");
}

#[test]
fn a_breakpoint_colour_is_rrggbb() {
    let mut value = base();
    value["breakpoints"][0]["color"] = json!("#22c55e");
    let config = validate(value).expect("valid");
    assert_eq!(config.breakpoints[0].color.as_deref(), Some("#22c55e"));
    assert_eq!(config.breakpoints[1].color, None);

    let mut value = base();
    value["breakpoints"][1]["color"] = json!("green");
    let issues = issues_of(value);
    assert_eq!(issues.len(), 1, "{issues:?}");
    assert_eq!(issues[0].path, "/breakpoints/1/color");
    assert_eq!(issues[0].key, "config.error.breakpoint_color");
}
