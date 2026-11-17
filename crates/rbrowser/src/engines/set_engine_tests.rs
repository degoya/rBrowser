use rb_commands::Binding;
use rb_config::{ConfigFile, EngineKind, Layout, LayoutMode};
use rb_engine::ViewId;
use rb_ipc::NotificationLevel;
use serde_json::{Value, json};

use super::*;
use crate::host::ui::recording::UiCall;
use crate::layout::test_support::{Setup, config, setup, setup_in, view_ids};

/// The test config on disk as `config.json` and a core over it.
fn project() -> (tempfile::TempDir, Setup) {
    project_from(serde_json::to_value(config()).expect("serializes"))
}

/// `value` on disk as `config.json` and a core over it.
fn project_from(value: Value) -> (tempfile::TempDir, Setup) {
    let dir = tempfile::tempdir().expect("a temporary folder");
    let root = dir.path().to_path_buf();
    std::fs::create_dir_all(root.join(".rbrowser")).expect("the config folder");
    rb_config::write_config_file(&root, ConfigFile::Project, &value).expect("the project file");
    let config = serde_json::from_value(value).expect("the test config parses");
    let setup = setup_in(config, Some(root));
    (dir, setup)
}

/// The newest notice of the engine switch in the status bar: its key and level.
fn last_notice(core: &Core) -> (String, NotificationLevel) {
    let message = core
        .status_log()
        .items()
        .into_iter()
        .rev()
        .find(|message| [WRITTEN_KEY, SHADOWED_KEY].contains(&message.message_key.as_str()))
        .expect("a notice of the engine switch");
    (message.message_key, message.level)
}

/// The test config with the canvas preset `items` (the owner's "4 bp", RB-090-74): a grid of
/// its own without cells, so its views are its canvas items only; `cells` as the top-level grid.
fn items_preset(cells: Option<Value>) -> Value {
    let mut value = serde_json::to_value(config()).expect("serializes");
    value["layouts"]["presets"]["items"] = json!({
        "mode": "canvas",
        "layout": { "columns": 1, "rows": 1, "gap": 8, "cells": [] },
        "items": [
            { "breakpoint": "mobile", "x": 0, "y": 20, "zoom": 0.5 },
            { "breakpoint": "desktop", "x": 600, "y": 20, "zoom": 0.5 }
        ]
    });
    if let Some(cells) = cells {
        value["layout"] = cells;
    }
    value
}

fn select_items(core: &Core) {
    crate::presets::run_command(core, "layout.select_preset", &json!({ "id": "items" }))
        .expect("a presets command")
        .expect("selects");
    assert_eq!(core.host().shell_state().mode, LayoutMode::Canvas);
}

/// The board's item of `view` in the canvas in effect; `None` when the board does not draw it.
fn board_item(core: &Core, view: ViewId) -> Option<rb_ipc::BoardItem> {
    let host = core.host();
    let spec = host.mode_spec()?;
    let area = host.grid_area()?;
    let session = host.shell_state().modes;
    crate::layout::canvas_board::board_state(host, &spec, area, &session)
        .items
        .into_iter()
        .find(|item| item.view == view)
}

/// The file's JSON; `null` when there is none.
fn file(dir: &tempfile::TempDir, file: ConfigFile) -> Value {
    let path = file.path(dir.path());
    if path.is_file() {
        rb_config::read_json(&path).expect("reads")
    } else {
        Value::Null
    }
}

fn engine_in(document: &Value, breakpoint: &str) -> Value {
    document["breakpoints"]
        .as_array()
        .and_then(|list| list.iter().find(|item| item["id"] == breakpoint))
        .map(|item| item["engine"].clone())
        .unwrap_or(Value::Null)
}

fn view_of(core: &Core, breakpoint: &str) -> ViewId {
    core.host()
        .views()
        .into_iter()
        .find(|view| view.cell_breakpoint.as_deref() == Some(breakpoint))
        .map(|view| view.id)
        .expect("the view exists")
}

#[test]
fn set_engine_patches_the_breakpoint_entry_and_adds_a_missing_one() {
    let mut document = json!({
        "breakpoints": [
            { "id": "mobile", "width": 400, "height": 800 },
            { "id": "desktop", "width": 1440, "height": 900, "engine": "chromium" }
        ]
    });
    patch_document(&mut document, "desktop", EngineKind::Firefox, None);
    assert_eq!(engine_in(&document, "desktop"), json!("firefox"));
    assert_eq!(engine_in(&document, "mobile"), Value::Null, "untouched");
    patch_document(&mut document, "tablet", EngineKind::Webkit, None);
    assert_eq!(
        document["breakpoints"][2],
        json!({ "id": "tablet", "engine": "webkit" }),
        "a local entry with only the engine; the merge joins it by id"
    );
    assert!(document.get("layout").is_none(), "no cells asked for");
    let mut empty = Value::Null;
    patch_document(&mut empty, "mobile", EngineKind::Chromium, None);
    assert_eq!(
        empty,
        json!({ "breakpoints": [{ "id": "mobile", "engine": "chromium" }] })
    );
}

#[test]
fn set_engine_writes_the_computed_cells_only_into_a_file_without_cells() {
    let layout: Layout = serde_json::from_value(json!({
        "columns": 2,
        "rows": 1,
        "gap": 12,
        "cells": [
            { "breakpoint": "mobile", "col": 1, "row": 1 },
            { "breakpoint": "desktop", "col": 2, "row": 1 }
        ]
    }))
    .expect("a layout");
    let mut document = json!({ "layout": { "gap": 20 } });
    patch_document(&mut document, "mobile", EngineKind::Firefox, Some(&layout));
    assert_eq!(document["layout"]["columns"], json!(2));
    assert_eq!(document["layout"]["rows"], json!(1));
    assert_eq!(
        document["layout"]["cells"].as_array().map(Vec::len),
        Some(2)
    );
    assert_eq!(
        document["layout"]["gap"],
        json!(20),
        "the file's own gap stays"
    );
    let mut own = json!({ "layout": { "columns": 1, "cells": [{ "breakpoint": "mobile", "col": 1, "row": 1 }] } });
    patch_document(&mut own, "mobile", EngineKind::Firefox, Some(&layout));
    assert_eq!(own["layout"]["columns"], json!(1), "own cells are kept");
    assert_eq!(own["layout"]["cells"].as_array().map(Vec::len), Some(1));
}

#[tokio::test]
async fn set_engine_prefers_the_shown_breakpoint_of_the_config_then_the_cells() {
    let setup = setup(config());
    let core = &setup.core;
    let mut info = core.host().view(view_of(core, "mobile")).expect("exists");
    assert_eq!(
        breakpoint_for(&core.host().config(), &info).as_deref(),
        Some("mobile")
    );
    info.breakpoint = Some("tablet".to_owned());
    assert_eq!(
        breakpoint_for(&core.host().config(), &info).as_deref(),
        Some("tablet")
    );
    info.breakpoint = Some("phone".to_owned());
    assert_eq!(
        breakpoint_for(&core.host().config(), &info).as_deref(),
        Some("mobile"),
        "a device preset is no breakpoint of the config: the cell's"
    );
    info.cell_breakpoint = None;
    assert_eq!(breakpoint_for(&core.host().config(), &info), None);
}

#[tokio::test]
async fn set_engine_writes_the_local_file_and_the_view_follows() {
    let (dir, setup) = project();
    let core = &setup.core;
    let mobile = view_of(core, "mobile");
    let answer = run(core, &json!({ "view": mobile, "engine": "firefox" })).expect("runs");
    assert_eq!(
        answer,
        json!({
            "view": mobile,
            "breakpoint": "mobile",
            "engine": "firefox",
            "file": "local",
            "changed": true,
        })
    );
    let local = file(&dir, ConfigFile::Local);
    assert_eq!(engine_in(&local, "mobile"), json!("firefox"));
    assert_eq!(
        local["layout"]["cells"].as_array().map(Vec::len),
        Some(3),
        "the project has no cells: the computed grid is written, so the view keeps its cell"
    );
    assert_eq!(
        engine_in(&file(&dir, ConfigFile::Project), "mobile"),
        json!("chromium"),
        "the project file is not touched (it was written with the explicit default)"
    );
    let shown = core.host().view(mobile).expect("the view stays");
    assert_eq!(shown.engine, EngineKind::Firefox);
    assert_eq!(core.host().views().len(), 3);
    assert_eq!(
        last_notice(core),
        (WRITTEN_KEY.to_owned(), NotificationLevel::Info),
        "the grid: \"runs on Firefox from now on\""
    );
    let again = run(core, &json!({ "view": mobile, "engine": "firefox" })).expect("runs");
    assert_eq!(again["changed"], json!(false));
}

/// The owner switched an engine on the canvas board and landed in grid mode: the config is
/// read again after the write, and the mode in effect must come back with it.
#[tokio::test]
async fn set_engine_keeps_the_layout_mode_in_effect() {
    let (_dir, setup) = project();
    let core = &setup.core;
    crate::layout::actions::run(core, crate::layout::defs::MODE_CANVAS, &Value::Null)
        .expect("canvas mode");
    assert_eq!(core.host().shell_state().mode, LayoutMode::Canvas);
    let mobile = core
        .host()
        .views()
        .into_iter()
        .find(|view| view.breakpoint.as_deref() == Some("mobile"))
        .map(|view| view.id)
        .expect("the mobile view");
    run(core, &json!({ "view": mobile, "engine": "firefox" })).expect("runs");
    assert_eq!(
        core.host().shell_state().mode,
        LayoutMode::Canvas,
        "the engine switch keeps the mode in effect"
    );
    assert!(core.host().mode_spec().is_some(), "the canvas spec is back");
}

/// RB-090-74: on a canvas whose views are its items only, a view switched to Firefox, WebKit
/// and back to Chromium keeps its board at its place - an external engine's view keeps its
/// Chromium stand-in, as in the grid - and every switch says "runs on ... from now on".
#[tokio::test]
async fn set_engine_on_the_canvas_keeps_the_board_for_every_engine() {
    let (_dir, setup) = project_from(items_preset(None));
    let core = &setup.core;
    select_items(core);
    let mobile = view_of(core, "mobile");
    let before = board_item(core, mobile).expect("the board draws mobile");
    for engine in ["firefox", "webkit", "chromium"] {
        let answer = run(core, &json!({ "view": mobile, "engine": engine })).expect("runs");
        assert_eq!(answer["changed"], json!(true), "{engine}");
        assert_eq!(
            last_notice(core),
            (WRITTEN_KEY.to_owned(), NotificationLevel::Info),
            "{engine}: no false warning"
        );
        let spec = core.host().mode_spec().expect("still the canvas");
        assert_eq!(spec.breakpoints, ["mobile", "desktop"], "{engine}");
        let shown = core
            .host()
            .view(mobile)
            .unwrap_or_else(|| panic!("{engine}: the view stays"));
        assert_eq!(config_name(shown.engine), engine);
        let item = board_item(core, mobile).unwrap_or_else(|| panic!("{engine}: the board stays"));
        assert_eq!(
            (item.label, item.rect),
            (before.label, before.rect),
            "{engine}: at its place"
        );
    }
}

/// RB-090-74: a canvas view whose breakpoint has no cell in the top-level grid opens anew
/// after the reload (another id); whether it changed is the read config's, not the old id's.
#[tokio::test]
async fn set_engine_reads_changed_from_the_config_when_the_view_opens_anew() {
    let grid = json!({
        "columns": 2,
        "rows": 1,
        "cells": [
            { "breakpoint": "desktop", "col": 1, "row": 1 },
            { "breakpoint": "tablet", "col": 2, "row": 1 }
        ]
    });
    let (_dir, setup) = project_from(items_preset(Some(grid)));
    let core = &setup.core;
    select_items(core);
    let mobile = view_of(core, "mobile");
    let answer = run(core, &json!({ "view": mobile, "engine": "firefox" })).expect("runs");
    assert_eq!(answer["changed"], json!(true));
    assert_eq!(
        last_notice(core),
        (WRITTEN_KEY.to_owned(), NotificationLevel::Info)
    );
    let now = core
        .host()
        .view(view_of(core, "mobile"))
        .expect("mobile has a view");
    assert_eq!(now.engine, EngineKind::Firefox);
    assert!(board_item(core, now.id).is_some(), "the board draws it");
}

/// A write into `config.json` that an entry of `config.local.json` shadows: the view keeps its
/// engine and the status bar says why.
#[tokio::test]
async fn set_engine_warns_when_the_local_file_shadows_the_project_write() {
    let (dir, setup) = project();
    rb_config::write_config_file(
        dir.path(),
        ConfigFile::Local,
        &json!({ "breakpoints": [{ "id": "mobile", "engine": "chromium" }] }),
    )
    .expect("the local file");
    let core = &setup.core;
    let mobile = view_of(core, "mobile");
    let answer = run(
        core,
        &json!({ "view": mobile, "engine": "firefox", "file": "project" }),
    )
    .expect("runs");
    assert_eq!(answer["changed"], json!(false));
    assert_eq!(
        engine_in(&file(&dir, ConfigFile::Project), "mobile"),
        json!("firefox")
    );
    assert_eq!(
        core.host().view(mobile).expect("the view stays").engine,
        EngineKind::Chromium
    );
    assert_eq!(
        last_notice(core),
        (SHADOWED_KEY.to_owned(), NotificationLevel::Warning)
    );
}

#[test]
fn the_notice_warns_only_when_the_local_file_shadows_a_project_write() {
    let written = Some((NotificationLevel::Info, WRITTEN_KEY));
    assert_eq!(notice_for(true, PresetFile::Local), written);
    assert_eq!(notice_for(true, PresetFile::Project), written);
    assert_eq!(
        notice_for(false, PresetFile::Project),
        Some((NotificationLevel::Warning, SHADOWED_KEY))
    );
    assert_eq!(
        notice_for(false, PresetFile::Local),
        None,
        "nothing shadows config.local.json"
    );
}

#[test]
fn the_effective_engine_is_the_read_configs() {
    let mut read = config();
    read.breakpoints[2].engine = EngineKind::Webkit;
    assert_eq!(effective_engine(&read, "mobile"), Some(EngineKind::Webkit));
    assert_eq!(
        effective_engine(&read, "desktop"),
        Some(EngineKind::Chromium)
    );
    assert_eq!(effective_engine(&read, "phone"), None);
}

#[tokio::test]
async fn set_engine_writes_the_project_file_when_asked() {
    let (dir, setup) = project();
    let core = &setup.core;
    let desktop = view_of(core, "desktop");
    core.host().set_active_view(desktop).expect("activates");
    let answer = run(core, &json!({ "engine": "webkit", "file": "project" })).expect("runs");
    assert_eq!(answer["view"], json!(desktop), "the active view");
    assert_eq!(answer["file"], json!("project"));
    assert_eq!(
        engine_in(&file(&dir, ConfigFile::Project), "desktop"),
        json!("webkit")
    );
    assert!(file(&dir, ConfigFile::Local).is_null());
    assert_eq!(
        core.host().view(desktop).expect("exists").engine,
        EngineKind::Webkit
    );
}

#[tokio::test]
async fn set_engine_without_an_engine_opens_the_menu_and_refuses_bad_arguments() {
    let setup = setup(config());
    let core = &setup.core;
    let ids = view_ids(&setup);
    core.host().set_active_view(ids[1]).expect("activates");
    assert_eq!(run(core, &Value::Null).expect("runs"), Value::Null);
    assert_eq!(
        setup.ui.calls().last(),
        Some(&UiCall::EnginePick(ids[1], None))
    );
    assert!(matches!(
        run(core, &json!({ "view": 999, "engine": "firefox" })),
        Err(CommandError::InvalidArgs(_))
    ));
    assert!(matches!(
        run(core, &json!({ "engine": "safari" })),
        Err(CommandError::InvalidArgs(_))
    ));
    assert!(matches!(
        run(core, &json!({ "engine": "firefox", "target": "all" })),
        Err(CommandError::InvalidArgs(_))
    ));
}

#[tokio::test]
async fn the_badge_switch_reaches_the_window() {
    let setup = setup(config());
    on_start(&setup.core);
    assert!(setup.ui.calls().contains(&UiCall::EngineBadges(true)));
    let mut user = setup.core.user_settings();
    user.ui.engine_badge = false;
    setup.core.replace_user_settings(user);
    // Other followers of the user settings (the app theme, RB-080-95) may call after it.
    assert!(setup.ui.calls().contains(&UiCall::EngineBadges(false)));
}

#[test]
fn set_engine_has_its_chord_and_its_tool() {
    let def = definition().expect("a definition");
    assert_eq!(def.id.as_str(), SET_ENGINE);
    assert_eq!(def.group, Group::Breakpoints);
    assert_eq!(def.default_bindings.len(), 1);
    assert!(matches!(&def.default_bindings[0], Binding::Leader(chord) if chord.keys.len() == 2));
    assert_eq!(def.mcp, McpCoverage::Covered(MCP_TOOL.to_owned()));
}
