use serde_json::{Map, Value, json};

use super::*;

fn object(value: Value) -> Map<String, Value> {
    value.as_object().cloned().expect("an object")
}

#[test]
fn every_overlay_is_found_by_its_id() {
    for id in ["overflow", "outlines", "highlight"] {
        assert_eq!(kind(id).map(|kind| kind.id), Some(id));
    }
    assert!(kind("all").is_none());
    assert!(kind("zindex").is_none());
}

#[test]
fn options_of_the_right_type_pass() {
    let highlight = kind("highlight").expect("highlight");
    let checked = validate_options(
        highlight,
        &object(json!({ "selector": "  .card  ", "index": -3 })),
    )
    .expect("valid");
    assert_eq!(checked["selector"], json!(".card"));
    assert_eq!(checked["index"], json!(-3));
}

#[test]
fn an_unknown_option_or_a_wrong_type_is_refused_by_name() {
    let outlines = kind("outlines").expect("outlines");
    let unknown =
        validate_options(outlines, &object(json!({ "depth": 3 }))).expect_err("unknown option");
    assert_eq!(unknown.code(), Some("plugin.overlays.errors.bad_option"));
    assert_eq!(
        unknown.params.get("option").map(String::as_str),
        Some("depth")
    );
    let wrong = validate_options(outlines, &object(json!({ "containersOnly": "yes" })))
        .expect_err("not a bool");
    assert_eq!(wrong.code(), Some("plugin.overlays.errors.bad_option"));
}

#[test]
fn a_selector_is_bounded_and_free_of_control_characters() {
    assert_eq!(selector(" main > .card "), Some("main > .card".to_owned()));
    assert_eq!(selector(".a\u{0}"), None);
    assert_eq!(selector(&"a".repeat(MAX_SELECTOR_CHARS + 1)), None);
    assert!(selector(&"a".repeat(MAX_SELECTOR_CHARS)).is_some());
}

#[test]
fn an_index_beyond_the_bound_is_refused() {
    let highlight = kind("highlight").expect("highlight");
    assert!(validate_options(highlight, &object(json!({ "index": MAX_INDEX + 1 }))).is_err());
    assert!(validate_options(highlight, &object(json!({ "index": 1.5 }))).is_err());
}

#[test]
fn the_further_overlays_of_rb_070_07_are_registered() {
    for id in [
        "stacking",
        "scrollbars",
        "text-spacing",
        "target-size",
        "small-text",
        "focus-tracking",
        "force-state",
        "content-chaos",
        "placeholdifier",
        "css-off",
        "js-off",
        "images-off",
    ] {
        assert_eq!(kind(id).map(|kind| kind.id), Some(id));
    }
    // And the tab order of RB-070-12, last.
    assert_eq!(OVERLAYS.last().map(|kind| kind.id), Some("tab-order"));
    assert_eq!(command_overlay("tab_order"), Some("tab-order"));
    assert_eq!(OVERLAYS.len(), 16);
}

#[test]
fn every_command_names_an_overlay_and_its_id_is_the_overlay_with_underscores() {
    for (command, overlay) in COMMANDS {
        assert!(kind(overlay).is_some(), "{command}");
        assert_eq!(command.replace('_', "-"), *overlay);
    }
    assert_eq!(command_overlay("target_size"), Some("target-size"));
    assert_eq!(command_overlay("off"), None);
    assert_eq!(command_overlay("target-size"), None);
}

#[test]
fn numbers_are_held_to_their_range() {
    let target = kind("target-size").expect("target-size");
    assert!(validate_options(target, &object(json!({ "sizePx": 44 }))).is_ok());
    assert!(validate_options(target, &object(json!({ "sizePx": 4 }))).is_err());
    assert!(validate_options(target, &object(json!({ "sizePx": 24.5 }))).is_err());
    let chaos = kind("content-chaos").expect("content-chaos");
    let checked = validate_options(chaos, &object(json!({ "factor": 0.5 }))).expect("valid");
    assert_eq!(checked["factor"], json!(0.5));
    assert!(validate_options(chaos, &object(json!({ "factor": 0 }))).is_err());
    assert!(validate_options(chaos, &object(json!({ "factor": 11 }))).is_err());
}

#[test]
fn forced_states_are_known_pseudo_classes_without_repeats() {
    let force = kind("force-state").expect("force-state");
    let checked = validate_options(
        force,
        &object(json!({ "selector": "nav a", "states": ["hover", "focus-visible"] })),
    )
    .expect("valid");
    assert_eq!(checked["states"], json!(["hover", "focus-visible"]));
    for bad in [
        json!([]),
        json!(["hover", "hover"]),
        json!(["before"]),
        json!("hover"),
        json!([1]),
    ] {
        assert!(
            validate_options(force, &object(json!({ "states": bad }))).is_err(),
            "{bad}"
        );
    }
    assert_eq!(force.requires, Some("selector"));
}
