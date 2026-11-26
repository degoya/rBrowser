use super::*;
use crate::validate::validate;
use serde_json::{Value, json};

fn config(extra: Value) -> ProjectConfig {
    let mut value = json!({
        "version": 1,
        "breakpoints": [
            { "id": "mobile", "width": 393, "height": 852 },
            { "id": "tablet", "width": 768, "height": 1024 },
            { "id": "desktop", "width": 1440, "height": 900 },
            { "id": "fox", "width": 1024, "height": 768, "engine": "firefox" }
        ]
    });
    if let (Some(base), Some(extra)) = (value.as_object_mut(), extra.as_object()) {
        for (key, field) in extra {
            base.insert(key.clone(), field.clone());
        }
    }
    validate(value).expect("a valid config")
}

fn resolve(config: &ProjectConfig, scope: ViewScope) -> Result<Vec<String>, ScopeError> {
    resolve_scope(config, &scope)
}

#[test]
fn the_scope_reads_and_writes_as_a_word_or_a_list() {
    for (text, scope) in [
        (json!("all"), ViewScope::All),
        (json!("visible"), ViewScope::Visible),
        (
            json!(["mobile", "desktop"]),
            ViewScope::Breakpoints(vec!["mobile".to_owned(), "desktop".to_owned()]),
        ),
    ] {
        let read: ViewScope = serde_json::from_value(text.clone()).expect("reads");
        assert_eq!(read, scope);
        assert_eq!(serde_json::to_value(&scope).expect("writes"), text);
    }
    assert!(serde_json::from_value::<ViewScope>(json!("hidden")).is_err());
    assert!(serde_json::from_value::<ViewScope>(json!(3)).is_err());
    assert_eq!(ViewScope::default(), ViewScope::All);
}

#[test]
fn all_is_every_breakpoint_in_config_order() {
    let config = config(json!({ "layout": { "hidden": ["tablet"] } }));
    assert_eq!(
        resolve(&config, ViewScope::All).expect("all"),
        ["mobile", "tablet", "desktop", "fox"]
    );
}

#[test]
fn visible_leaves_out_the_hidden_selection() {
    let config = config(json!({ "layout": { "hidden": ["tablet", "fox"] } }));
    assert_eq!(
        resolve(&config, ViewScope::Visible).expect("visible"),
        ["mobile", "desktop"]
    );
}

#[test]
fn visible_takes_a_presets_hidden_once_it_is_the_selection() {
    // Choosing a preset writes its `hidden` as `layout.hidden` (RB-080-41); the scope reads the
    // selection in effect, so a preset's choice counts once it is shown.
    let mut config = config(json!({ "layouts": { "presets": {
        "phones": { "mode": "grid", "hidden": ["desktop", "tablet"] }
    } } }));
    assert_eq!(
        resolve(&config, ViewScope::Visible).expect("before"),
        ["mobile", "tablet", "desktop", "fox"]
    );
    config.layout.hidden = config.layouts.presets["phones"]
        .hidden
        .clone()
        .expect("the preset's hidden");
    assert_eq!(
        resolve(&config, ViewScope::Visible).expect("after"),
        ["mobile", "fox"]
    );
}

#[test]
fn visible_in_a_grid_with_cells_is_only_the_breakpoints_in_a_cell() {
    let config = config(json!({ "layout": {
        "columns": 2, "rows": 1,
        "cells": [
            { "breakpoint": "desktop", "col": 1, "row": 1 },
            { "breakpoint": "mobile", "col": 2, "row": 1 }
        ],
        "hidden": ["mobile"]
    } }));
    assert_eq!(
        resolve(&config, ViewScope::Visible).expect("visible"),
        ["desktop"]
    );
}

#[test]
fn a_selection_that_would_empty_the_grid_hides_nothing() {
    // `hidden_selection` keeps a Chromium view open: hiding every Chromium breakpoint hides none.
    let config = config(json!({ "layout": { "hidden": ["mobile", "tablet", "desktop"] } }));
    assert_eq!(
        resolve(&config, ViewScope::Visible).expect("visible"),
        ["mobile", "tablet", "desktop", "fox"]
    );
}

#[test]
fn visible_may_hold_no_chromium_breakpoint_the_caller_filters_engines() {
    let config = config(json!({ "layout": {
        "cells": [{ "breakpoint": "fox", "col": 1, "row": 1 }]
    } }));
    assert_eq!(
        resolve(&config, ViewScope::Visible).expect("visible"),
        ["fox"]
    );
}

#[test]
fn named_breakpoints_are_checked_and_kept_in_config_order() {
    let config = config(json!({}));
    let named = ViewScope::Breakpoints(vec![
        "desktop".to_owned(),
        "mobile".to_owned(),
        "desktop".to_owned(),
    ]);
    assert_eq!(
        resolve(&config, named).expect("named"),
        ["mobile", "desktop"]
    );
    let unknown = ViewScope::Breakpoints(vec!["mobile".to_owned(), "watch".to_owned()]);
    let error = resolve(&config, unknown).expect_err("unknown");
    assert_eq!(error, ScopeError::UnknownBreakpoint("watch".to_owned()));
    assert_eq!(error.code(), "scope.error.unknown_breakpoint");
}

#[test]
fn an_empty_result_is_an_error() {
    let empty = ProjectConfig::default();
    for scope in [
        ViewScope::All,
        ViewScope::Visible,
        ViewScope::Breakpoints(Vec::new()),
    ] {
        let error = resolve(&empty, scope).expect_err("empty");
        assert_eq!(error, ScopeError::Empty);
        assert_eq!(error.code(), "scope.error.empty");
    }
    let named_none = ViewScope::Breakpoints(Vec::new());
    assert_eq!(
        resolve(&config(json!({})), named_none),
        Err(ScopeError::Empty)
    );
}
