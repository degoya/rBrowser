use serde_json::json;

use super::*;

fn mode_of(preset: serde_json::Value) -> LayoutMode {
    serde_json::from_value::<LayoutPreset>(preset)
        .expect("a preset")
        .mode
}

/// RB-099-26 review: a hand-written preset with its own grid and no `mode` was a grid preset
/// by the built-in mode before - it stays one; a preset with neither is the canvas.
#[test]
fn a_preset_without_a_mode_is_a_grid_with_its_own_layout_else_a_canvas() {
    let layout = json!({ "columns": 2, "rows": 1, "cells": [
        { "breakpoint": "mobile", "col": 1, "row": 1 },
        { "breakpoint": "desktop", "col": 2, "row": 1 }
    ] });
    assert_eq!(mode_of(json!({ "layout": layout })), LayoutMode::Grid);
    assert_eq!(mode_of(json!({})), LayoutMode::Canvas);
    assert_eq!(mode_of(json!({ "hidden": ["tablet"] })), LayoutMode::Canvas);
    assert_eq!(
        mode_of(json!({ "mode": "canvas", "layout": layout })),
        LayoutMode::Canvas
    );
    assert_eq!(
        mode_of(json!({ "mode": "focus", "layout": layout })),
        LayoutMode::Focus
    );
}

#[test]
fn every_field_reads_and_writes_back_with_its_mode() {
    let preset = json!({
        "mode": "canvas", "snap": 8, "focus": "mobile", "hidden": [],
        "items": [{ "breakpoint": "mobile", "x": 0, "y": 20, "zoom": 0.5 }]
    });
    let read: LayoutPreset = serde_json::from_value(preset.clone()).expect("a preset");
    assert_eq!(read.modes.snap, Some(8));
    assert_eq!(read.modes.items.len(), 1);
    assert_eq!(read.focus.as_deref(), Some("mobile"));
    assert_eq!(read.hidden, Some(Vec::new()));
    assert_eq!(serde_json::to_value(&read).expect("writes"), preset);
    // A preset read without `mode` is written with the one it was read as.
    let grid: LayoutPreset =
        serde_json::from_value(json!({ "layout": { "columns": 1 } })).expect("a preset");
    assert_eq!(serde_json::to_value(&grid).expect("writes")["mode"], "grid");
}
