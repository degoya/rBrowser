use serde_json::{Value, json};

use super::{ApplyArgs, ProposedBreakpoint, apply};

fn proposed(id: &str, width: u32, is_new: bool) -> ProposedBreakpoint {
    ProposedBreakpoint {
        id: id.to_owned(),
        label: id.to_owned(),
        width,
        height: 800,
        is_new,
    }
}

fn ids(breakpoints: &[Value]) -> Vec<&str> {
    breakpoints
        .iter()
        .filter_map(|breakpoint| breakpoint["id"].as_str())
        .collect()
}

#[test]
fn replacing_writes_the_chosen_views_and_a_grid() {
    let applied = apply(&ApplyArgs {
        existing: vec![json!({ "id": "old", "width": 500, "height": 900 })],
        layout: json!({ "gap": 12, "cells": [] }),
        proposed: (1..=6)
            .map(|n| proposed(&format!("b{n}"), n * 200, true))
            .collect(),
        selected: Vec::new(),
        replace: true,
        ..ApplyArgs::default()
    });
    assert_eq!(
        ids(&applied.breakpoints),
        vec!["b1", "b2", "b3", "b4", "b5", "b6"]
    );
    assert_eq!(applied.layout["columns"], 4);
    assert_eq!(applied.layout["rows"], 2);
    assert_eq!(applied.layout["gap"], 12);
    assert_eq!(
        applied.layout["cells"][5],
        json!({ "breakpoint": "b6", "col": 2, "row": 2 })
    );
}

#[test]
fn appending_keeps_the_existing_and_skips_known_widths() {
    let existing =
        json!({ "id": "md", "width": 768, "height": 1024, "touch": true, "device": "ipad" });
    let applied = apply(&ApplyArgs {
        existing: vec![existing.clone()],
        layout: Value::Null,
        proposed: vec![
            proposed("sm", 640, true),
            proposed("md", 768, false),
            proposed("lg", 1024, true),
        ],
        selected: Vec::new(),
        replace: false,
        ..ApplyArgs::default()
    });
    assert_eq!(applied.breakpoints[0], existing);
    assert_eq!(ids(&applied.breakpoints), vec!["md", "sm", "lg"]);
    assert_eq!(applied.added, vec!["sm", "lg"]);
    assert_eq!(applied.layout, Value::Null);
}

#[test]
fn a_taken_id_gets_the_width() {
    let applied = apply(&ApplyArgs {
        existing: vec![json!({ "id": "md", "width": 800, "height": 1024 })],
        proposed: vec![proposed("md", 768, true)],
        selected: vec!["md".to_owned()],
        ..ApplyArgs::default()
    });
    assert_eq!(applied.added, vec!["md-768"]);
}

#[test]
fn an_existing_grid_gets_a_row_below() {
    let layout = json!({
        "columns": 3, "rows": 1, "gap": 8,
        "cells": [{ "breakpoint": "a", "col": 1, "row": 1 }]
    });
    let applied = apply(&ApplyArgs {
        existing: vec![json!({ "id": "a", "width": 400, "height": 700 })],
        layout,
        proposed: vec![proposed("sm", 640, true)],
        selected: Vec::new(),
        replace: false,
        ..ApplyArgs::default()
    });
    assert_eq!(applied.layout["rows"], 2);
    assert_eq!(applied.layout["columns"], 3);
    assert_eq!(
        applied.layout["cells"][1],
        json!({ "breakpoint": "sm", "col": 1, "row": 2 })
    );
}

#[test]
fn nothing_chosen_adds_nothing() {
    let applied = apply(&ApplyArgs {
        proposed: vec![proposed("md", 768, false)],
        ..ApplyArgs::default()
    });
    assert!(applied.added.is_empty());
    assert!(applied.breakpoints.is_empty());
}

#[test]
fn a_width_a_device_preset_gives_an_existing_breakpoint_is_taken() {
    // `phone` names a preset and has no `width` of its own; the host resolved it to 393.
    let applied = apply(&ApplyArgs {
        existing: vec![json!({ "id": "phone", "device": "iphone-15" })],
        existing_widths: vec![393],
        proposed: vec![proposed("sm", 393, true), proposed("md", 768, true)],
        ..ApplyArgs::default()
    });
    assert_eq!(applied.added, vec!["md"]);
    assert_eq!(ids(&applied.breakpoints), vec!["phone", "md"]);
}
