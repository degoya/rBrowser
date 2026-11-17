use super::*;
use serde_json::{Value, json};

/// The plugin's settings schema, as the host reads it.
fn schema() -> Value {
    serde_json::from_str(include_str!("../settings.schema.json")).expect("the settings schema")
}

#[test]
fn the_defaults_are_the_colours_the_overlay_always_drew() {
    let colors = Colors::default();
    assert_eq!(colors.ruler, "#f6f7f9f0");
    assert_eq!(colors.ticks, "#4a4f5c");
    assert_eq!(colors.guide, "#00a2ff");
    assert_eq!(colors.column_fill, "#ff40811f");
    assert_eq!(colors.column_edge, "#ff408100");
    assert_eq!(colors.baseline, "#00a2ff59");
}

#[test]
fn every_colour_is_a_setting_with_its_default_and_annotations() {
    let schema = schema();
    let properties = &schema["properties"]["colors"]["properties"];
    let defaults = serde_json::to_value(Colors::default()).expect("the defaults");
    let defaults = defaults.as_object().expect("the colours are an object");
    assert_eq!(
        properties.as_object().map(|keys| keys.len()),
        Some(defaults.len()),
        "one setting per colour"
    );
    for (key, default) in defaults {
        let field = &properties[key];
        assert_eq!(&field["default"], default, "default of colors.{key}");
        assert_eq!(field["format"], "color", "colors.{key} is a colour field");
        assert_eq!(field["x-category"], "devtools");
        assert_eq!(field["x-section"], "rulers");
        assert!(field["x-order"].is_u64(), "x-order of colors.{key}");
        assert_eq!(field["x-scope"], json!(["user", "project", "local"]));
        let pattern = field["pattern"].as_str().expect("a pattern");
        assert_eq!(pattern, COLOR_PATTERN);
    }
}

#[test]
fn a_colour_is_six_or_eight_hex_digits() {
    for good in ["#00a2ff", "#00A2FF", "#00a2ff59", "#FFFFFF00"] {
        assert!(is_color(good), "{good}");
    }
    for bad in [
        "",
        "00a2ff",
        "#0af",
        "#00a2f",
        "#00a2ff5",
        "#00a2ff590",
        "#00a2fg",
        "red",
        "rgba(0,0,0,1)",
        "#00a2ff;}",
    ] {
        assert!(!is_color(bad), "{bad}");
    }
}

#[test]
fn an_override_wins_per_colour() {
    let overrides = ColorOverrides {
        guide: Some("#ff0000".to_owned()),
        baseline: Some("#00000080".to_owned()),
        ..ColorOverrides::default()
    };
    let shared = Colors {
        ruler: "#202020".to_owned(),
        ..Colors::default()
    };
    let colors = overrides.over(&shared);
    assert_eq!(colors.guide, "#ff0000");
    assert_eq!(colors.baseline, "#00000080");
    assert_eq!(colors.ruler, "#202020");
    assert_eq!(colors.ticks, Colors::default().ticks);
    assert!(!overrides.is_empty());
    assert!(ColorOverrides::default().is_empty());
}

#[test]
fn an_override_that_is_no_colour_is_refused() {
    let bad = ColorOverrides {
        column_edge: Some("blue".to_owned()),
        ..ColorOverrides::default()
    };
    assert_eq!(bad.check(), Err("colors.columnEdge"));
    let good = ColorOverrides {
        column_edge: Some("#0000ff80".to_owned()),
        ..ColorOverrides::default()
    };
    assert_eq!(good.check(), Ok(()));
}

#[test]
fn the_wire_form_is_camel_case_and_leaves_out_what_is_not_overridden() {
    let overrides: ColorOverrides =
        serde_json::from_value(json!({"columnFill": "#11223344"})).expect("overrides");
    assert_eq!(overrides.column_fill.as_deref(), Some("#11223344"));
    assert_eq!(
        serde_json::to_value(&overrides).expect("json"),
        json!({"columnFill": "#11223344"})
    );
    assert!(serde_json::from_value::<ColorOverrides>(json!({"text": "#000000"})).is_err());
    let colors = serde_json::to_value(Colors::default()).expect("json");
    assert_eq!(colors["columnEdge"], "#ff408100");
}
