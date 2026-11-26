use super::*;
use crate::validate::validate;
use crate::{CURRENT_VERSION, ConfigError, Layout, LayoutMode, ProjectConfig, migrate};
use serde_json::{Value, json};

fn base() -> Value {
    json!({
        "version": 1,
        "breakpoints": [
            { "id": "mobile", "width": 393, "height": 852 },
            { "id": "tablet", "width": 768, "height": 1024 },
            { "id": "desktop", "width": 1440, "height": 900 }
        ],
        "layouts": { "presets": {
            "strip": { "mode": "horizontal", "height": "fit", "gap": 12 },
            "pair": { "mode": "split", "orientation": "horizontal", "ratio": 0.6,
                      "primary": ["desktop"], "secondary": ["mobile", "tablet"] },
            "board": { "mode": "canvas", "snap": 8, "items": [
                { "breakpoint": "mobile", "x": 0, "y": 0, "zoom": 0.5 },
                { "breakpoint": "desktop", "x": 400, "y": -16 }
            ] }
        } }
    })
}

fn issues_of(value: Value) -> Vec<ConfigIssue> {
    match validate(value) {
        Err(ConfigError::Invalid(issues)) => issues,
        other => panic!("expected issues, got {other:?}"),
    }
}

#[test]
fn the_three_modes_deserialize_with_their_fields() {
    let config = validate(base()).expect("valid");
    let strip = &config.layouts.presets["strip"];
    assert_eq!(strip.mode, LayoutMode::Horizontal);
    assert_eq!(strip.modes.height, Some(StripHeight::Fit));
    assert_eq!(strip.modes.gap, Some(12));

    let pair = &config.layouts.presets["pair"];
    assert_eq!(pair.mode, LayoutMode::Split);
    assert_eq!(pair.modes.orientation, Some(SplitOrientation::Horizontal));
    assert_eq!(pair.modes.ratio, Some(0.6));
    assert_eq!(pair.modes.primary, ["desktop"]);
    assert_eq!(pair.modes.secondary, ["mobile", "tablet"]);

    let board = &config.layouts.presets["board"];
    assert_eq!(board.mode, LayoutMode::Canvas);
    assert_eq!(board.modes.snap, Some(8));
    assert_eq!(board.modes.items.len(), 2);
    assert_eq!(board.modes.items[1].y, -16);
    assert_eq!(
        board.modes.items[1].zoom,
        default_item_zoom(),
        "zoom defaults"
    );
}

#[test]
fn a_fixed_height_reads_and_writes_as_a_number() {
    let height: StripHeight = serde_json::from_value(json!(640)).expect("number");
    assert_eq!(height, StripHeight::Px(640));
    assert_eq!(
        serde_json::to_value(height).expect("serializes"),
        json!(640)
    );
    assert_eq!(
        serde_json::to_value(StripHeight::Fit).expect("serializes"),
        json!("fit")
    );
}

#[test]
fn a_preset_of_version_1_keeps_its_form_and_needs_no_migration() {
    let value = json!({
        "version": 1,
        "breakpoints": [{ "id": "mobile", "width": 393, "height": 852 }],
        "layouts": { "default": "focus", "presets": {
            "grid-main": { "mode": "grid" },
            "focus": { "mode": "focus", "focus": "mobile" }
        } }
    });
    let (migrated, notes) = migrate::migrate(value.clone()).expect("migrates");
    assert!(notes.is_empty(), "no step for the new modes: {notes:?}");
    assert_eq!(
        CURRENT_VERSION, 1,
        "the new fields are optional, no version bump"
    );
    let config: ProjectConfig = validate(migrated).expect("valid");
    let written = serde_json::to_value(&config.layouts).expect("serializes");
    assert_eq!(
        written, value["layouts"],
        "no mode field is written when unset"
    );
}

#[test]
fn a_ratio_outside_the_range_is_reported() {
    let mut value = base();
    value["layouts"]["presets"]["pair"]["ratio"] = json!(0.95);
    let issues = issues_of(value);
    assert_eq!(issues.len(), 1, "{issues:?}");
    assert_eq!(issues[0].path, "/layouts/presets/pair/ratio");
    assert_eq!(issues[0].key, "config.error.ratio_range");
}

#[test]
fn a_breakpoint_on_both_sides_or_unknown_is_reported() {
    let mut value = base();
    value["layouts"]["presets"]["pair"]["secondary"] = json!(["desktop", "watch"]);
    let issues = issues_of(value);
    let found: Vec<(&str, &str)> = issues
        .iter()
        .map(|issue| (issue.path.as_str(), issue.key.as_str()))
        .collect();
    assert_eq!(
        found,
        [
            (
                "/layouts/presets/pair/secondary/0",
                "config.error.duplicate_breakpoint"
            ),
            (
                "/layouts/presets/pair/secondary/1",
                "config.error.unknown_breakpoint"
            ),
        ]
    );
}

#[test]
fn canvas_items_are_checked_for_breakpoint_zoom_and_range() {
    let mut value = base();
    value["layouts"]["presets"]["board"]["items"] = json!([
        { "breakpoint": "mobile", "x": 0, "y": 0, "zoom": 1.5 },
        { "breakpoint": "mobile", "x": 200_000, "y": 0 }
    ]);
    let issues = issues_of(value);
    let keys: Vec<&str> = issues.iter().map(|issue| issue.key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "config.error.zoom_range",
            "config.error.duplicate_breakpoint",
            "config.error.canvas_range"
        ],
        "{issues:?}"
    );
}

#[test]
fn a_height_gap_or_snap_out_of_range_is_reported() {
    let mut value = base();
    value["layouts"]["presets"]["strip"]["height"] = json!(20);
    value["layouts"]["presets"]["strip"]["gap"] = json!(500);
    value["layouts"]["presets"]["board"]["snap"] = json!(129);
    let issues = issues_of(value);
    let paths: Vec<&str> = issues.iter().map(|issue| issue.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "/layouts/presets/strip/height",
            "/layouts/presets/strip/gap",
            "/layouts/presets/board/snap"
        ]
    );
}

#[test]
fn the_top_level_layout_keeps_its_modes_fields() {
    let mut value = base();
    value["layout"] = json!({ "modes": {
        "split": { "ratio": 0.25, "primary": ["mobile"] },
        "canvas": { "items": [{ "breakpoint": "tablet", "x": 8, "y": 16 }] }
    } });
    let config = validate(value).expect("valid");
    let modes = &config.layout.modes;
    assert_eq!(
        modes.get(LayoutMode::Split).and_then(|f| f.ratio),
        Some(0.25)
    );
    assert_eq!(
        modes.get(LayoutMode::Canvas).map(|f| f.items.len()),
        Some(1)
    );
    assert!(modes.get(LayoutMode::Horizontal).is_none());
    assert!(modes.get(LayoutMode::Grid).is_none(), "grid has no fields");
    let written = serde_json::to_value(&config).expect("serializes");
    assert_eq!(written["layout"]["modes"]["split"]["ratio"], json!(0.25));
    assert!(written["layout"]["modes"].get("horizontal").is_none());
}

#[test]
fn a_layout_without_modes_writes_none() {
    let config = validate(base()).expect("valid");
    assert!(config.layout.modes.is_empty());
    let written = serde_json::to_value(&config).expect("serializes");
    assert!(written["layout"].get("modes").is_none(), "{written}");
}

#[test]
fn the_top_level_modes_are_checked() {
    let mut value = base();
    value["layout"] = json!({ "modes": { "split": { "ratio": 2.0, "primary": ["phone"] } } });
    let paths: Vec<String> = issues_of(value).into_iter().map(|i| i.path).collect();
    assert!(
        paths.contains(&"/layout/modes/split/ratio".to_owned()),
        "{paths:?}"
    );
    assert!(
        paths.contains(&"/layout/modes/split/primary/0".to_owned()),
        "{paths:?}"
    );
}

#[test]
fn set_stores_a_modes_fields() {
    let mut modes = LayoutModes::default();
    let fields = ModeFields {
        gap: Some(4),
        ..ModeFields::default()
    };
    assert!(modes.set(LayoutMode::Horizontal, fields.clone()));
    assert_eq!(modes.get(LayoutMode::Horizontal), Some(&fields));
    assert!(!modes.set(LayoutMode::Focus, fields), "focus has no fields");
}

/// `layout.hidden` (RB-080-41): the breakpoints whose views are not open, in every mode; a
/// preset keeps its own selection, absent when it was saved without one.
#[test]
fn the_hidden_selection_reads_and_writes_in_the_layout_and_in_a_preset() {
    let mut value = base();
    value["layout"] = json!({ "hidden": ["tablet"] });
    value["layouts"]["presets"]["pair"]["hidden"] = json!([]);
    let config = validate(value).expect("valid");
    assert_eq!(config.layout.hidden, ["tablet"]);
    assert_eq!(config.layouts.presets["pair"].hidden, Some(Vec::new()));
    assert_eq!(config.layouts.presets["board"].hidden, None);
    let written = serde_json::to_value(&config).expect("serializes");
    assert_eq!(written["layout"]["hidden"], json!(["tablet"]));
    assert_eq!(written["layouts"]["presets"]["pair"]["hidden"], json!([]));
    assert!(
        written["layouts"]["presets"]["board"]
            .get("hidden")
            .is_none(),
        "{written}"
    );
    let plain = validate(base()).expect("valid");
    let written = serde_json::to_value(&plain).expect("serializes");
    assert!(written["layout"].get("hidden").is_none(), "{written}");
}

#[test]
fn a_hidden_breakpoint_must_exist_and_is_listed_once() {
    let mut value = base();
    value["layout"] = json!({ "hidden": ["tablet", "phablet", "tablet"] });
    value["layouts"]["presets"]["strip"]["hidden"] = json!(["nope"]);
    let issues = issues_of(value);
    let paths: Vec<(&str, &str)> = issues
        .iter()
        .map(|issue| (issue.path.as_str(), issue.key.as_str()))
        .collect();
    assert_eq!(
        paths,
        [
            ("/layout/hidden/1", "config.error.unknown_breakpoint"),
            ("/layout/hidden/2", "config.error.duplicate_breakpoint"),
            (
                "/layouts/presets/strip/hidden/0",
                "config.error.unknown_breakpoint"
            ),
        ]
    );
}

#[test]
fn the_open_breakpoints_are_the_known_ones_not_hidden() {
    let ids = ["mobile", "tablet", "desktop"];
    let known: HashSet<&str> = ids.into_iter().collect();
    assert_eq!(
        hidden_of(&["tablet".to_owned(), "ghost".to_owned()], &known),
        ["tablet"]
    );
    assert!(hidden_of(&[], &known).is_empty());
}

/// The selection never hides the last Chromium view: a config that lists every breakpoint
/// opens them all.
#[test]
fn the_selection_keeps_at_least_one_chromium_view_open() {
    let mut value = base();
    value["layout"] = json!({ "hidden": ["mobile", "tablet", "desktop"] });
    let config = validate(value).expect("valid");
    assert!(hidden_selection(&config).is_empty());
    let mut value = base();
    value["layout"] = json!({ "hidden": ["mobile", "tablet"] });
    let config = validate(value).expect("valid");
    assert_eq!(hidden_selection(&config), ["mobile", "tablet"]);
}

/// The grid in effect decides what "the last view" is: an explicit grid shows only its cells.
#[test]
fn the_selection_keeps_a_cell_of_the_grid_in_effect_open() {
    let hidden = ["mobile".to_owned()];
    let mut value = base();
    value["layout"] = json!({ "columns": 1, "rows": 1, "hidden": ["mobile"],
        "cells": [{ "breakpoint": "mobile", "col": 1, "row": 1 }] });
    let config = validate(value).expect("valid");
    assert!(!keeps_a_chromium_view(&config, &hidden));
    assert!(hidden_selection(&config).is_empty());
    let mut value = base();
    value["layout"] = json!({ "columns": 2, "rows": 1, "hidden": ["mobile"],
        "cells": [
            { "breakpoint": "mobile", "col": 1, "row": 1 },
            { "breakpoint": "tablet", "col": 2, "row": 1 }
        ] });
    let config = validate(value).expect("valid");
    assert!(keeps_a_chromium_view(&config, &hidden));
    assert_eq!(hidden_selection(&config), ["mobile"]);
    // A preset's grid in effect instead of the config's.
    let solo: Layout = serde_json::from_value(json!({ "columns": 1, "rows": 1,
        "cells": [{ "breakpoint": "mobile", "col": 1, "row": 1 }] }))
    .expect("a grid");
    assert!(!keeps_a_chromium_view_in(&config, &solo, &hidden));
    assert!(hidden_selection_in(&config, &solo).is_empty());
    // A computed grid: every Chromium breakpoint has a cell.
    let computed = Layout::default();
    assert!(keeps_a_chromium_view_in(&config, &computed, &hidden));
    assert!(!keeps_a_chromium_view_in(
        &config,
        &computed,
        &[
            "mobile".to_owned(),
            "tablet".to_owned(),
            "desktop".to_owned()
        ]
    ));
}

/// RB-099-26 (owner, 2026-11-26): canvas is the built-in layout mode - a preset that names no
/// mode is a canvas; a config that names one keeps it.
#[test]
fn a_preset_without_a_mode_is_a_canvas_and_a_named_mode_stays() {
    assert_eq!(LayoutMode::default(), LayoutMode::Canvas);
    let config: ProjectConfig = serde_json::from_value(json!({
        "version": 1,
        "layouts": { "presets": {
            "free": {},
            "cells": { "mode": "grid" },
            "phone": { "mode": "focus" }
        } }
    }))
    .expect("parses");
    let presets = &config.layouts.presets;
    assert_eq!(presets["free"].mode, LayoutMode::Canvas);
    assert_eq!(presets["cells"].mode, LayoutMode::Grid);
    assert_eq!(presets["phone"].mode, LayoutMode::Focus);
    // Written back, every preset names its mode: a later default changes none of them.
    let written = serde_json::to_value(&config).expect("serializes");
    assert_eq!(written["layouts"]["presets"]["free"]["mode"], "canvas");
    assert_eq!(written["layouts"]["presets"]["cells"]["mode"], "grid");
}

/// The schema says what a preset without `mode` is (RB-099-26).
#[test]
fn the_schema_names_canvas_as_the_default_mode() {
    let schema = Value::from(crate::project_schema());
    let mode = &schema["properties"]["layouts"]["properties"]["presets"]["additionalProperties"]["properties"]
        ["mode"];
    assert_eq!(mode["default"], "canvas", "{mode}");
}
