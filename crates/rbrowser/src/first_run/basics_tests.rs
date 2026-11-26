use serde_json::json;

use super::*;

#[test]
fn canvas_adds_a_canvas_preset_and_starts_with_it() {
    let mut config = json!({ "version": 1, "breakpoints": [] });
    apply(&mut config, Some(SetupLayout::Canvas), None);
    assert_eq!(
        config["layouts"],
        json!({ "default": "canvas", "presets": { "canvas": { "mode": "canvas" } } })
    );
}

#[test]
fn canvas_reuses_a_canvas_preset_the_project_has() {
    let mut config = json!({
        "layouts": { "presets": { "board": { "mode": "canvas" }, "two": { "mode": "grid" } } }
    });
    apply(&mut config, Some(SetupLayout::Canvas), None);
    assert_eq!(config["layouts"]["default"], "board");
    assert!(config["layouts"]["presets"].get("canvas").is_none());
}

/// RB-099-26: the top-level layout starts in the canvas now, so a grid choice names the grid -
/// a grid preset the project has, else a new one - as the canvas choice names the canvas.
#[test]
fn grid_starts_with_a_grid_preset_the_project_has_or_a_new_one() {
    let mut config = json!({
        "layouts": { "default": "board", "presets": { "board": { "mode": "canvas" } } }
    });
    apply(&mut config, Some(SetupLayout::Grid), None);
    assert_eq!(config["layouts"]["default"], "grid");
    assert_eq!(
        config["layouts"]["presets"]["grid"],
        json!({ "mode": "grid" })
    );
    assert_eq!(config["layouts"]["presets"]["board"]["mode"], "canvas");
    // A grid preset the project has is reused.
    let mut two = json!({ "layouts": { "presets": { "two": { "mode": "grid" } } } });
    apply(&mut two, Some(SetupLayout::Grid), None);
    assert_eq!(two["layouts"]["default"], "two");
    assert!(two["layouts"]["presets"].get("grid").is_none());
}

/// A preset without `mode` and without a grid of its own is in the built-in mode, the canvas
/// (RB-099-26); a preset of another mode keeps its name - the new one takes the next free one.
#[test]
fn a_preset_without_a_mode_is_a_canvas_and_a_taken_name_is_not_written_over() {
    let mut config = json!({ "layouts": { "presets": { "free": {} } } });
    apply(&mut config, Some(SetupLayout::Canvas), None);
    assert_eq!(config["layouts"]["default"], "free");
    let mut taken = json!({ "layouts": { "presets": { "grid": { "mode": "focus" } } } });
    apply(&mut taken, Some(SetupLayout::Grid), None);
    assert_eq!(taken["layouts"]["default"], "grid-2");
    assert_eq!(taken["layouts"]["presets"]["grid"]["mode"], "focus");
    assert_eq!(
        taken["layouts"]["presets"]["grid-2"],
        json!({ "mode": "grid" })
    );
}

/// RB-099-26 review: a grid preset with its own few cells (`phones`: two mobile views) as the
/// start would open the project with those alone - the grid choice names a grid of every view.
/// A preset without `mode` but with its own `layout` is such a grid preset too.
#[test]
fn grid_skips_a_grid_preset_with_its_own_cells() {
    let cells = json!({ "columns": 2, "rows": 1, "cells": [
        { "breakpoint": "mobile", "col": 1, "row": 1 },
        { "breakpoint": "mobile-wide", "col": 2, "row": 1 }
    ] });
    for phones in [
        json!({ "mode": "grid", "layout": cells }),
        json!({ "layout": cells }),
    ] {
        let mut config = json!({ "layouts": { "presets": { "phones": phones } } });
        apply(&mut config, Some(SetupLayout::Grid), None);
        assert_eq!(config["layouts"]["default"], "grid", "{phones}");
        assert_eq!(
            config["layouts"]["presets"]["grid"],
            json!({ "mode": "grid" })
        );
        assert_eq!(config["layouts"]["presets"]["phones"], phones);
    }
    // Nor a canvas of hidden views for the canvas choice.
    let mut config = json!({ "layouts": { "presets": {
        "some": { "mode": "canvas", "hidden": ["tablet"] }
    } } });
    apply(&mut config, Some(SetupLayout::Canvas), None);
    assert_eq!(config["layouts"]["default"], "canvas");
}

#[test]
fn sync_off_switches_every_input_off_and_on_goes_back_to_the_defaults() {
    let mut config = json!({ "sync": { "hoverThrottleMs": 50 } });
    apply(&mut config, None, Some(false));
    for key in SYNC_KEYS {
        assert_eq!(config["sync"][key], false, "{key}");
    }
    assert_eq!(config["sync"]["hoverThrottleMs"], 50);
    apply(&mut config, None, Some(true));
    assert_eq!(config["sync"], json!({ "hoverThrottleMs": 50 }));
}

#[test]
fn nothing_chosen_changes_nothing() {
    let mut config = json!({ "version": 1 });
    apply(&mut config, None, None);
    assert_eq!(config, json!({ "version": 1 }));
}
