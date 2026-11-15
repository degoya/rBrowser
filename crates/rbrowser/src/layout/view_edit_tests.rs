use rb_config::{ConfigFile, LayoutMode};
use serde_json::{Value, json};

use super::*;
use crate::breakpoint_keys::FreeSize;
use crate::host::ui::recording::UiCall;
use crate::layout::test_support::{Setup, config, setup_in};

/// The test config on disk as `config.json` and a core over it.
fn project() -> (tempfile::TempDir, Setup) {
    project_with(config())
}

fn project_with(config: ProjectConfig) -> (tempfile::TempDir, Setup) {
    let dir = tempfile::tempdir().expect("a temporary folder");
    let root = dir.path().to_path_buf();
    std::fs::create_dir_all(root.join(".rbrowser")).expect("the config folder");
    let value = serde_json::to_value(&config).expect("serializes");
    rb_config::write_config_file(&root, ConfigFile::Project, &value).expect("the project file");
    let setup = setup_in(config, Some(root));
    (dir, setup)
}

fn ok(core: &Core, id: &str, args: Value) -> Value {
    run(core, id, &args).unwrap_or_else(|error| panic!("{id}: {error}"))
}

/// The local file's JSON; `null` when there is none.
fn local(dir: &tempfile::TempDir) -> Value {
    let path = ConfigFile::Local.path(dir.path());
    if path.is_file() {
        rb_config::read_json(&path).expect("reads")
    } else {
        Value::Null
    }
}

/// The breakpoints of the open views, in grid order.
fn open(core: &Core) -> Vec<String> {
    core.host()
        .views()
        .into_iter()
        .filter_map(|view| view.cell_breakpoint)
        .collect()
}

#[tokio::test]
async fn remove_hides_the_breakpoint_in_the_local_file_and_add_opens_it_again() {
    let (dir, setup) = project();
    let core = &setup.core;
    let tablet = core
        .host()
        .views()
        .into_iter()
        .find(|view| view.cell_breakpoint.as_deref() == Some("tablet"))
        .expect("tablet")
        .id;
    let answer = ok(core, REMOVE, json!({ "view": tablet }));
    assert_eq!(answer["removed"], json!("tablet"));
    assert_eq!(answer["hidden"], json!(["tablet"]));
    assert_eq!(answer["changed"], json!(true));
    assert_eq!(open(core), ["desktop", "mobile"]);
    assert_eq!(local(&dir)["layout"]["hidden"], json!(["tablet"]));
    assert!(core.host().view(tablet).is_none(), "the view is closed");
    // Removing it again changes nothing.
    let again = ok(core, REMOVE, json!({ "breakpoint": "tablet" }));
    assert_eq!(again["changed"], json!(false));
    let answer = ok(core, ADD, json!({ "breakpoint": "tablet" }));
    assert_eq!(answer["added"], json!(["tablet"]));
    assert_eq!(answer["hidden"], json!([]));
    assert_eq!(open(core), ["desktop", "tablet", "mobile"]);
    assert!(
        local(&dir)["layout"].get("hidden").is_none(),
        "an empty selection is not written"
    );
}

#[tokio::test]
async fn the_last_view_cannot_be_removed() {
    let (_dir, setup) = project();
    let core = &setup.core;
    ok(core, REMOVE, json!({ "breakpoint": "desktop" }));
    ok(core, REMOVE, json!({ "breakpoint": "mobile" }));
    assert_eq!(open(core), ["tablet"]);
    let error = run(core, REMOVE, &Value::Null).expect_err("refused");
    assert!(error.to_string().contains("last view"), "{error}");
    assert_eq!(open(core), ["tablet"]);
    let all = ok(core, ADD, json!({ "breakpoint": ALL }));
    assert_eq!(all["added"], json!(["desktop", "mobile"]));
    assert_eq!(open(core), ["desktop", "tablet", "mobile"]);
}

#[tokio::test]
async fn an_unknown_breakpoint_is_refused_and_no_arguments_open_the_shell_menu() {
    let (_dir, setup) = project();
    let core = &setup.core;
    let error = run(core, REMOVE, &json!({ "breakpoint": "phablet" })).expect_err("refused");
    assert!(matches!(error, CommandError::InvalidArgs(_)), "{error}");
    assert_eq!(ok(core, ADD, Value::Null), Value::Null);
    assert_eq!(ok(core, ADD_SIZE, Value::Null), Value::Null);
    let scripts: Vec<String> = setup
        .ui
        .calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::Shell(script) => Some(script),
            _ => None,
        })
        .collect();
    assert!(
        scripts
            .iter()
            .any(|script| script.contains("uiCommand") && script.contains(ADD)),
        "{scripts:?}"
    );
    assert!(
        scripts.iter().any(|script| script.contains(ADD_SIZE)),
        "{scripts:?}"
    );
}

#[tokio::test]
async fn add_size_writes_a_breakpoint_of_that_size_and_opens_it() {
    let (dir, setup) = project();
    let core = &setup.core;
    let answer = ok(core, ADD_SIZE, json!({ "width": 1024, "height": 768 }));
    assert_eq!(answer["breakpoint"], json!("size-1024x768"));
    assert_eq!(answer["added"], json!(true));
    assert_eq!(open(core), ["desktop", "tablet", "mobile", "size-1024x768"]);
    let written = &local(&dir)["breakpoints"][0];
    assert_eq!(written["id"], json!("size-1024x768"));
    assert_eq!(written["width"], json!(1024));
    assert_eq!(written["height"], json!(768));
    assert!(
        local(&dir)["layout"].get("cells").is_none(),
        "a computed grid flows by itself"
    );
    // The same size once more: nothing is added, the breakpoint is open.
    let again = ok(core, ADD_SIZE, json!({ "width": 1024, "height": 768 }));
    assert_eq!(again["added"], json!(false));
    assert_eq!(open(core).len(), 4);
    // A name of its own and a DPR.
    let named = ok(
        core,
        ADD_SIZE,
        json!({ "width": 800, "height": 600, "name": "retro", "dpr": 2 }),
    );
    assert_eq!(named["breakpoint"], json!("retro"));
    assert_eq!(
        local(&dir)["breakpoints"][1]["emulation"]["dpr"],
        json!(2.0)
    );
    assert_eq!(open(core).len(), 5);
    let bad = run(core, ADD_SIZE, &json!({ "width": 0, "height": 10 })).expect_err("refused");
    assert!(matches!(bad, CommandError::InvalidArgs(_)), "{bad}");
    let bad = run(
        core,
        ADD_SIZE,
        &json!({ "width": 10, "height": 10, "name": "a/b" }),
    )
    .expect_err("refused");
    assert!(matches!(bad, CommandError::InvalidArgs(_)), "{bad}");
}

#[tokio::test]
async fn an_explicit_grid_gets_a_column_for_the_new_size_and_keeps_an_empty_cell() {
    let mut config = config();
    config.layout = crate::host::state::layout_for(&config);
    let (dir, setup) = project_with(config);
    let core = &setup.core;
    ok(core, REMOVE, json!({ "breakpoint": "tablet" }));
    assert_eq!(open(core), ["desktop", "mobile"]);
    let grid = setup.ui.last_grid().expect("a grid");
    assert_eq!(grid.cells.len(), 2);
    assert!(
        matches!(&grid.plan.kind, crate::layout_plan::NodeKind::Split { children, .. }
            if children[1].kind == crate::layout_plan::NodeKind::Hidden("tablet".to_owned())),
        "{:?}",
        grid.plan
    );
    ok(core, ADD_SIZE, json!({ "width": 1024, "height": 768 }));
    assert_eq!(open(core), ["desktop", "mobile", "size-1024x768"]);
    let written = local(&dir);
    assert_eq!(written["layout"]["columns"], json!(4));
    assert_eq!(written["layout"]["cells"].as_array().map(Vec::len), Some(4));
    assert_eq!(written["layout"]["hidden"], json!(["tablet"]));
}

#[tokio::test]
async fn on_the_canvas_a_view_is_added_at_the_click() {
    let (_dir, setup) = project();
    let core = &setup.core;
    ok(core, REMOVE, json!({ "breakpoint": "mobile" }));
    crate::layout::actions::run(core, crate::layout::defs::MODE_CANVAS, &Value::Null)
        .expect("canvas");
    assert_eq!(core.host().shell_state().mode, LayoutMode::Canvas);
    let answer = ok(
        core,
        ADD,
        // On the snap grid (8 DIP), as every canvas placement is.
        json!({ "breakpoint": "mobile", "x": 1000, "y": 504 }),
    );
    assert_eq!(answer["added"], json!(["mobile"]));
    assert_eq!(core.host().shell_state().mode, LayoutMode::Canvas);
    let spec = core.host().mode_spec().expect("the canvas stays");
    let crate::layout::spec::ModeKind::Canvas { items, .. } = &spec.kind else {
        panic!("{spec:?}");
    };
    let placed = items
        .iter()
        .find(|item| item.breakpoint == "mobile")
        .expect("placed");
    assert_eq!((placed.x, placed.y), (1000, 504));
    let bad = run(core, ADD, &json!({ "breakpoint": "mobile", "x": 1 })).expect_err("refused");
    assert!(matches!(bad, CommandError::InvalidArgs(_)), "{bad}");
}

#[tokio::test]
async fn a_preset_saved_with_a_selection_restores_it_and_one_without_leaves_it() {
    let (dir, setup) = project();
    let core = &setup.core;
    ok(core, REMOVE, json!({ "breakpoint": "tablet" }));
    crate::layout::actions::run(
        core,
        crate::layout::defs::SAVE_PRESET,
        &json!({ "id": "two" }),
    )
    .expect("saves");
    assert_eq!(
        local(&dir)["layouts"]["presets"]["two"]["hidden"],
        json!(["tablet"])
    );
    // The saved grid holds every cell, the hidden one's slot included.
    assert_eq!(
        local(&dir)["layouts"]["presets"]["two"]["layout"]["cells"]
            .as_array()
            .map(Vec::len),
        Some(3)
    );
    ok(core, ADD, json!({ "breakpoint": ALL }));
    assert_eq!(open(core).len(), 3);
    crate::presets::run_command(core, "layout.select_preset", &json!({ "id": "two" }))
        .expect("a presets command")
        .expect("selects");
    assert_eq!(open(core), ["desktop", "mobile"]);
    assert_eq!(core.host().shell_state().preset.as_deref(), Some("two"));
    // A preset without a selection: the two stay.
    crate::presets::run_command(core, "layout.select_preset", &json!({ "id": "pair" }))
        .expect("a presets command")
        .expect("selects");
    assert_eq!(open(core), ["desktop", "mobile"]);
    assert_eq!(core.host().shell_state().mode, LayoutMode::Split);
    // A restart reads the selection from the local file.
    let root = dir.path().to_path_buf();
    let loaded = crate::project::load(&root).expect("loads");
    assert_eq!(rb_config::hidden_selection(&loaded), ["tablet"]);
}

#[test]
fn the_definitions_are_in_the_view_group_with_a_tool_each() {
    let defs = definitions().expect("definitions");
    let ids: Vec<&str> = defs.iter().map(|def| def.id.as_str()).collect();
    assert_eq!(ids, [ADD, REMOVE, ADD_SIZE]);
    for def in &defs {
        assert_eq!(def.group, Group::View);
        assert!(matches!(&def.mcp, McpCoverage::Covered(tool) if tool.starts_with("view_")));
        assert!(is_own(def.id.as_str()));
    }
    assert!(!is_own("view.zoom_in"));
}

#[test]
fn a_breakpoint_name_is_short_and_plain() {
    assert_eq!(breakpoint_id("  wide-2 ").expect("ok"), "wide-2");
    assert!(breakpoint_id("").is_err());
    assert!(breakpoint_id("a b").is_err());
    assert!(breakpoint_id(&"x".repeat(41)).is_err());
}

#[test]
fn add_breakpoint_joins_by_id() {
    let mut document = json!({ "breakpoints": [{ "id": "a", "engine": "firefox" }] });
    let size = FreeSize::new(500, 800, Some(2.0)).expect("a size");
    add_breakpoint(&mut document, "a", &size);
    assert_eq!(document["breakpoints"].as_array().map(Vec::len), Some(1));
    add_breakpoint(&mut document, "b", &size);
    assert_eq!(
        document["breakpoints"][1],
        json!({ "id": "b", "width": 500, "height": 800, "emulation": { "dpr": 2.0 } })
    );
    let mut empty = Value::Null;
    add_breakpoint(&mut empty, "c", &size);
    assert_eq!(empty["breakpoints"][0]["id"], json!("c"));
}

#[tokio::test]
async fn in_focus_mode_remove_hides_the_focused_breakpoint_and_focus_moves_on() {
    let (_dir, setup) = project();
    let core = &setup.core;
    crate::presets::run_command(core, "layout.focus_mode", &json!({ "enabled": true }))
        .expect("a presets command")
        .expect("focus mode");
    assert_eq!(core.host().shell_state().mode, LayoutMode::Focus);
    assert_eq!(open(core), ["desktop"]);
    let answer = ok(core, REMOVE, json!({ "breakpoint": "desktop" }));
    assert_eq!(answer["hidden"], json!(["desktop"]));
    assert_eq!(core.host().shell_state().mode, LayoutMode::Focus);
    assert_eq!(open(core), ["tablet"]);
    // The active view, the focused one.
    ok(core, REMOVE, Value::Null);
    assert_eq!(open(core), ["mobile"]);
    let error = run(core, REMOVE, &Value::Null).expect_err("the last view");
    assert!(error.to_string().contains("last view"), "{error}");
}

/// An explicit grid shows only its cells: a hidden breakpoint without one is not offered and
/// not added; `view.add_size` with its name gives it a column.
#[tokio::test]
async fn a_breakpoint_without_a_cell_in_the_explicit_grid_is_not_offered_but_add_size_gives_it_one()
{
    let mut config = config();
    config.layout = serde_json::from_value(json!({
        "columns": 2, "rows": 1, "hidden": ["mobile"],
        "cells": [
            { "breakpoint": "desktop", "col": 1, "row": 1 },
            { "breakpoint": "tablet", "col": 2, "row": 1 }
        ]
    }))
    .expect("a layout");
    let (dir, setup) = project_with(config);
    let core = &setup.core;
    assert_eq!(open(core), ["desktop", "tablet"]);
    assert!(addable(core.host()).is_empty());
    let error = run(core, ADD, &json!({ "breakpoint": "mobile" })).expect_err("no cell");
    assert!(matches!(error, CommandError::InvalidArgs(_)), "{error}");
    let all = ok(core, ADD, json!({ "breakpoint": ALL }));
    assert_eq!(all["changed"], json!(false));
    let answer = ok(
        core,
        ADD_SIZE,
        json!({ "width": 400, "height": 800, "name": "mobile" }),
    );
    assert_eq!(answer["added"], json!(false));
    assert_eq!(open(core), ["desktop", "tablet", "mobile"]);
    assert_eq!(
        local(&dir)["layout"]["cells"].as_array().map(Vec::len),
        Some(3)
    );
    ok(core, REMOVE, json!({ "breakpoint": "mobile" }));
    assert_eq!(addable(core.host()), ["mobile"]);
}

#[tokio::test]
async fn an_ad_hoc_view_is_closed_by_remove() {
    let (_dir, setup) = project();
    let core = &setup.core;
    let mut ad_hoc = config().breakpoints[2].clone();
    ad_hoc.id = "ad-hoc".to_owned();
    let view = core.host().open_view(&ad_hoc).expect("opens");
    assert_eq!(core.host().views().len(), 4);
    let answer = ok(core, REMOVE, json!({ "view": view }));
    assert_eq!(answer["removed"], json!("ad-hoc"));
    assert_eq!(answer["changed"], json!(true));
    assert_eq!(answer["hidden"], json!([]));
    assert!(core.host().view(view).is_none());
    assert_eq!(open(core), ["desktop", "tablet", "mobile"]);
}

/// A preset whose grid leaves out every open breakpoint shows its cells rather than nothing.
#[tokio::test]
async fn a_preset_grid_that_would_show_no_view_shows_its_cells() {
    let mut config = config();
    config.layout.hidden = vec!["mobile".to_owned()];
    config.layouts.presets.insert(
        "solo".to_owned(),
        serde_json::from_value(json!({
            "mode": "grid",
            "layout": { "columns": 1, "rows": 1,
                "cells": [{ "breakpoint": "mobile", "col": 1, "row": 1 }] }
        }))
        .expect("a preset"),
    );
    let (_dir, setup) = project_with(config);
    let core = &setup.core;
    assert_eq!(open(core), ["desktop", "tablet"]);
    crate::presets::run_command(core, "layout.select_preset", &json!({ "id": "solo" }))
        .expect("a presets command")
        .expect("selects");
    assert_eq!(open(core), ["mobile"]);
    let error = run(core, REMOVE, &json!({ "breakpoint": "mobile" })).expect_err("the last view");
    assert!(error.to_string().contains("last view"), "{error}");
}

#[tokio::test]
async fn choosing_a_preset_compares_the_selection_as_a_set_and_applies_it_once() {
    let mut config = config();
    for (id, hidden) in [
        ("rev", json!(["mobile", "tablet"])),
        ("one", json!(["desktop"])),
    ] {
        config.layouts.presets.insert(
            id.to_owned(),
            serde_json::from_value(json!({ "mode": "grid", "hidden": hidden })).expect("a preset"),
        );
    }
    let (dir, setup) = project_with(config);
    let core = &setup.core;
    ok(core, REMOVE, json!({ "breakpoint": "tablet" }));
    ok(core, REMOVE, json!({ "breakpoint": "mobile" }));
    assert_eq!(local(&dir)["layout"]["hidden"], json!(["tablet", "mobile"]));
    let grids = || {
        setup
            .ui
            .calls()
            .iter()
            .filter(|call| matches!(call, UiCall::Grid(_)))
            .count()
    };
    let before = grids();
    crate::presets::run_command(core, "layout.select_preset", &json!({ "id": "rev" }))
        .expect("a presets command")
        .expect("selects");
    // The same selection in another order: nothing written, the grid built once at most.
    assert_eq!(local(&dir)["layout"]["hidden"], json!(["tablet", "mobile"]));
    assert!(grids() - before <= 1, "{} grids", grids() - before);
    let before = grids();
    crate::presets::run_command(core, "layout.select_preset", &json!({ "id": "one" }))
        .expect("a presets command")
        .expect("selects");
    assert_eq!(open(core), ["tablet", "mobile"]);
    assert_eq!(core.host().shell_state().preset.as_deref(), Some("one"));
    // The config read again and the new preset applied; the old one is not shown in between.
    assert!(grids() - before <= 2, "{} grids", grids() - before);
}
