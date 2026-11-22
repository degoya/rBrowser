use super::*;
use crate::validate::validate;
use serde_json::{Value, json};

/// A project file that names `tablet` everywhere a breakpoint id may stand.
fn project() -> Value {
    json!({
        "version": 1,
        "breakpoints": [
            { "id": "mobile", "width": 393, "height": 852 },
            { "id": "tablet", "width": 768, "height": 1024 },
            { "id": "desktop", "width": 1440, "height": 900 }
        ],
        "layout": {
            "columns": 3, "rows": 1,
            "cells": [
                { "breakpoint": "mobile", "col": 1, "row": 1 },
                { "breakpoint": "tablet", "col": 2, "row": 1 },
                { "breakpoint": "tablet", "col": 3, "row": 1 }
            ],
            "hidden": ["tablet"],
            "modes": {
                "split": { "primary": ["tablet"], "secondary": ["mobile", "desktop"] },
                "canvas": { "items": [
                    { "breakpoint": "tablet", "x": 0, "y": 0 },
                    { "breakpoint": "desktop", "x": 400, "y": 0 }
                ] }
            }
        },
        "layouts": { "presets": {
            "pair": { "mode": "grid", "layout": { "columns": 2, "rows": 1, "cells": [
                { "breakpoint": "tablet", "col": 1, "row": 1 },
                { "breakpoint": "desktop", "col": 2, "row": 1 }
            ] }, "hidden": ["tablet", "mobile"] },
            "big": { "mode": "focus", "focus": "tablet" },
            "side": { "mode": "split", "primary": ["desktop"], "secondary": ["tablet"] },
            "board": { "mode": "canvas", "items": [{ "breakpoint": "tablet", "x": 0, "y": 0 }] }
        } },
        "envCompare": { "breakpoints": ["tablet", "desktop"] }
    })
}

fn without_breakpoint(mut document: Value, id: &str) -> Value {
    if let Some(list) = document["breakpoints"].as_array_mut() {
        list.retain(|breakpoint| breakpoint["id"] != id);
    }
    document
}

#[test]
fn every_reference_leaves_one_file_and_is_counted() {
    let mut document = project();
    let found = remove_references(&mut document, "tablet");
    // Two cells of the grid and one of the preset `pair`.
    assert_eq!(found.cells, 3);
    // `layout.hidden` and the preset's `hidden`.
    assert_eq!(found.hidden, 2);
    // Split primary, canvas item, preset focus, preset split and canvas, envCompare.
    assert_eq!(found.other, 6);
    assert_eq!(found.total(), 11);
    assert_eq!(
        document["layout"]["cells"],
        json!([{ "breakpoint": "mobile", "col": 1, "row": 1 }])
    );
    assert_eq!(document["layout"]["hidden"], json!([]));
    assert_eq!(document["layout"]["modes"]["split"]["primary"], json!([]));
    assert_eq!(
        document["layout"]["modes"]["canvas"]["items"],
        json!([{ "breakpoint": "desktop", "x": 400, "y": 0 }])
    );
    let presets = &document["layouts"]["presets"];
    assert_eq!(presets["pair"]["hidden"], json!(["mobile"]));
    assert!(presets["big"].get("focus").is_none(), "{presets}");
    assert_eq!(presets["side"]["secondary"], json!([]));
    assert_eq!(presets["board"]["items"], json!([]));
    assert_eq!(document["envCompare"]["breakpoints"], json!(["desktop"]));
    // The breakpoint itself is the caller's to remove.
    assert_eq!(document["breakpoints"].as_array().map(Vec::len), Some(3));
}

#[test]
fn a_file_without_references_is_left_as_it_is() {
    let mut document = json!({ "breakpoints": [{ "id": "mobile" }], "layout": { "gap": 4 } });
    let before = document.clone();
    assert_eq!(
        remove_references(&mut document, "tablet"),
        References::default()
    );
    assert_eq!(document, before);
}

#[test]
fn the_config_validates_after_a_breakpoint_and_its_references_left() {
    // Before the fix the settings window refused exactly this (`/layout/cells/1/breakpoint`).
    let before = defined_ids(&project(), None);
    let mut document = without_breakpoint(project(), "tablet");
    assert!(
        validate(document.clone()).is_err(),
        "the dangling references"
    );
    let found = drop_removed(&before, &mut document, None);
    assert_eq!(found.total(), 11);
    validate(document).expect("valid without the references");
}

#[test]
fn references_leave_the_other_file_too() {
    let project_file = project();
    let local_file = json!({
        "layout": { "hidden": ["tablet", "mobile"] },
        "envCompare": { "breakpoints": ["tablet"] }
    });
    let before = defined_ids(&project_file, Some(&local_file));
    let mut project_file = without_breakpoint(project_file, "tablet");
    let mut local = local_file;
    let found = drop_removed(&before, &mut project_file, Some(&mut local));
    assert_eq!(found.total(), 13);
    assert_eq!(local["layout"]["hidden"], json!(["mobile"]));
    assert_eq!(local["envCompare"]["breakpoints"], json!([]));
    let mut merged = project_file;
    crate::merge(&mut merged, local);
    validate(merged).expect("valid merged");
}

#[test]
fn an_id_the_other_file_still_defines_keeps_its_references() {
    let project_file = project();
    let local_file = json!({ "breakpoints": [{ "id": "tablet", "width": 800, "height": 1000 }] });
    let before = defined_ids(&project_file, Some(&local_file));
    let mut edited = without_breakpoint(project_file, "tablet");
    let untouched = edited.clone();
    let mut local = local_file;
    assert_eq!(
        drop_removed(&before, &mut edited, Some(&mut local)),
        References::default()
    );
    assert_eq!(edited, untouched);
}

#[test]
fn removing_every_breakpoint_by_a_reset_drops_every_reference() {
    let before = defined_ids(&project(), None);
    let mut document = project();
    if let Some(object) = document.as_object_mut() {
        object.remove("breakpoints");
    }
    let found = drop_removed(&before, &mut document, None);
    assert_eq!(found.cells, 5);
    assert_eq!(document["layout"]["cells"], json!([]));
}

#[test]
fn defined_ids_reads_both_files() {
    let ids = defined_ids(
        &json!({ "breakpoints": [{ "id": "a" }, { "id": "b" }, { "width": 3 }] }),
        Some(&json!({ "breakpoints": [{ "id": "c" }] })),
    );
    assert_eq!(
        ids.into_iter().collect::<Vec<_>>(),
        vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]
    );
}

/// RB-094-25: the merge joins the two files' `breakpoints` by id, so a breakpoint goes only when
/// no file defines it any more - its entry leaves both.
#[test]
fn a_definition_leaves_both_files() {
    let mut project_file = project();
    let mut local = json!({
        "breakpoints": [
            { "id": "tablet", "width": 800, "height": 1000 },
            { "id": "local-only", "width": 1600, "height": 1000 }
        ]
    });
    let gone: BTreeSet<String> = ["tablet".to_owned()].into_iter().collect();
    assert_eq!(
        remove_definitions(&gone, &mut project_file, Some(&mut local)),
        2
    );
    let after = defined_ids(&project_file, Some(&local));
    assert!(!after.contains("tablet"), "{after:?}");
    assert!(after.contains("local-only"));
    assert!(after.contains("mobile"));
    assert_eq!(
        local["breakpoints"],
        json!([{ "id": "local-only", "width": 1600, "height": 1000 }])
    );
    assert_eq!(remove_definitions(&gone, &mut project_file, None), 0);
}
