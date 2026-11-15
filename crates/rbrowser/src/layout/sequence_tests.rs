//! The owner's sequence (build 22481e2): a project with a top-level grid and no presets of its
//! own, split mode, the divider moved, the config read again after that write, then every other
//! layout control. Each must still switch the layout.

use rb_config::{ConfigFile, LayoutMode, ProjectConfig};
use serde_json::{Value, json};

use super::actions::run;
use super::defs::{MODE_CANVAS, MODE_GRID, MODE_HORIZONTAL, MODE_SPLIT};
use super::spec::ModeKind;
use super::test_support::{Setup, setup_in, view_ids};
use crate::control::Core;
use crate::host::ui::recording::UiCall;

/// Three breakpoints in a top-level 3 x 1 grid, no presets.
fn owner_config() -> ProjectConfig {
    serde_json::from_value(json!({
        "version": 1,
        "startUrl": "http://localhost:3000/",
        "breakpoints": [
            { "id": "mobile", "width": 400, "height": 800 },
            { "id": "tablet", "width": 768, "height": 1024 },
            { "id": "desktop", "width": 1440, "height": 900 }
        ],
        "layout": { "columns": 3, "rows": 1, "gap": 8, "cells": [
            { "breakpoint": "mobile", "col": 1, "row": 1 },
            { "breakpoint": "tablet", "col": 2, "row": 1 },
            { "breakpoint": "desktop", "col": 3, "row": 1 }
        ] }
    }))
    .expect("the owner's config parses")
}

/// The project on disk and a core over it.
fn project() -> (tempfile::TempDir, Setup) {
    let dir = tempfile::tempdir().expect("a temporary folder");
    let root = dir.path().to_path_buf();
    std::fs::create_dir_all(root.join(".rbrowser")).expect("the config folder");
    let value = serde_json::to_value(owner_config()).expect("serializes");
    rb_config::write_config_file(&root, ConfigFile::Project, &value).expect("the project file");
    let setup = setup_in(owner_config(), Some(root));
    (dir, setup)
}

fn ok(core: &Core, id: &str, args: Value) -> Value {
    run(core, id, &args).unwrap_or_else(|error| panic!("{id}: {error}"))
}

fn presets_command(core: &Core, id: &str, args: Value) {
    crate::presets::run_command(core, id, &args)
        .expect("a presets command")
        .unwrap_or_else(|error| panic!("{id}: {error:?}"));
}

/// What the watcher does after the write: the config read again and applied.
fn reload(core: &Core) {
    crate::commands::reload_config(core.host()).expect("reads the config again");
    crate::presets::on_config_changed(core);
}

fn mode(core: &Core) -> LayoutMode {
    core.host().shell_state().mode
}

/// The divider, then the reload, then every mode and focus mode still switch.
#[tokio::test]
async fn after_the_divider_and_the_reload_every_mode_still_switches() {
    let (_dir, setup) = project();
    let core = &setup.core;
    let views = view_ids(&setup);
    ok(core, MODE_SPLIT, Value::Null);
    ok(core, MODE_SPLIT, json!({ "ratio": 0.25 }));
    reload(core);
    assert_eq!(mode(core), LayoutMode::Split);
    assert!(matches!(
        core.host().mode_spec().map(|spec| spec.kind),
        Some(ModeKind::Split { ratio, .. }) if ratio == 0.25
    ));

    ok(core, MODE_GRID, Value::Null);
    assert_eq!(mode(core), LayoutMode::Grid);
    assert!(core.host().mode_spec().is_none(), "grid places no views");
    assert_eq!(view_ids(&setup), views);

    ok(core, MODE_HORIZONTAL, Value::Null);
    assert_eq!(mode(core), LayoutMode::Horizontal);
    assert!(matches!(
        core.host().mode_spec().map(|spec| spec.kind),
        Some(ModeKind::Horizontal { .. })
    ));

    presets_command(core, "layout.focus_mode", Value::Null);
    assert_eq!(mode(core), LayoutMode::Focus);
    presets_command(core, "layout.focus_mode", Value::Null);
    assert_eq!(
        mode(core),
        LayoutMode::Horizontal,
        "focus off returns to the mode"
    );

    // Focus mode closed the other views and opened them again.
    let views = view_ids(&setup);
    assert_eq!(views.len(), 3);
    ok(core, MODE_SPLIT, Value::Null);
    assert!(
        matches!(
            core.host().mode_spec().map(|spec| spec.kind),
            Some(ModeKind::Split { ratio, .. }) if ratio == 0.25
        ),
        "the divider's ratio is kept for the split"
    );

    ok(core, MODE_CANVAS, Value::Null);
    assert_eq!(mode(core), LayoutMode::Canvas);
    ok(core, MODE_GRID, Value::Null);
    assert_eq!(mode(core), LayoutMode::Grid);
    assert_eq!(view_ids(&setup), views);
}

/// The divider writes no preset: the select stays empty and grid is the top-level layout.
#[tokio::test]
async fn the_divider_writes_no_preset() {
    let (dir, setup) = project();
    let core = &setup.core;
    ok(core, MODE_SPLIT, Value::Null);
    let answer = ok(core, MODE_SPLIT, json!({ "ratio": 0.25 }));
    reload(core);
    let config = core.host().config();
    assert!(
        config.layouts.presets.is_empty(),
        "{:?}",
        config.layouts.presets
    );
    assert_eq!(core.host().shell_state().preset, None);
    assert_eq!(answer["persisted"], json!("layout"));
    let local = rb_config::read_json(&ConfigFile::Local.path(dir.path())).expect("the local file");
    assert_eq!(local["layout"]["modes"]["split"]["ratio"], json!(0.25));
    assert!(local.get("layouts").is_none(), "{local}");
}

/// The divider in split mode moves the views: no rebuild of the grid (every overlay closed and
/// made again), only new places.
#[tokio::test]
async fn the_divider_places_the_views_without_a_rebuild() {
    let (_dir, setup) = project();
    let core = &setup.core;
    ok(core, MODE_SPLIT, Value::Null);
    let grids = |setup: &Setup| {
        setup
            .ui
            .calls()
            .iter()
            .filter(|call| matches!(call, UiCall::Grid(_)))
            .count()
    };
    let before = grids(&setup);
    let answer = ok(core, MODE_SPLIT, json!({ "ratio": 0.3 }));
    assert_eq!(grids(&setup), before, "no grid rebuilt");
    assert_eq!(answer["mode"], json!("split"));
    assert_eq!(answer["persisted"], json!("layout"));
    assert!(matches!(
        core.host().mode_spec().map(|spec| spec.kind),
        Some(ModeKind::Split { ratio, .. }) if ratio == 0.3
    ));
}
