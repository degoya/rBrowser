use rb_commands::CommandError;
use rb_config::{ConfigFile, LayoutMode};
use serde_json::{Value, json};

use super::super::actions::run;
use super::super::defs::{MODE_CANVAS, MODE_SPLIT, SAVE_PRESET};
use super::super::test_support::{Setup, config, setup_in};
use super::*;
use crate::host::ui::recording::UiCall;

/// The test config on disk as `config.json` and a core over it.
fn project() -> (tempfile::TempDir, Setup) {
    let dir = tempfile::tempdir().expect("a temporary folder");
    let root = dir.path().to_path_buf();
    std::fs::create_dir_all(root.join(".rbrowser")).expect("the config folder");
    let value = serde_json::to_value(config()).expect("serializes");
    rb_config::write_config_file(&root, ConfigFile::Project, &value).expect("the project file");
    let setup = setup_in(config(), Some(root));
    (dir, setup)
}

fn ok(core: &Core, id: &str, args: Value) -> Value {
    run(core, id, &args).unwrap_or_else(|error| panic!("{id}: {error}"))
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

fn preset<'a>(document: &'a Value, id: &str) -> &'a Value {
    &document["layouts"]["presets"][id]
}

#[tokio::test]
async fn the_canvas_is_saved_with_every_view_to_the_local_file_and_selected() {
    let (dir, setup) = project();
    let core = &setup.core;
    ok(core, MODE_CANVAS, Value::Null);
    let answer = ok(core, SAVE_PRESET, json!({ "id": "  my board " }));
    assert_eq!(
        answer,
        json!({ "id": "my board", "file": "local", "mode": "canvas" })
    );
    let local = file(&dir, ConfigFile::Local);
    let saved = preset(&local, "my board");
    assert_eq!(saved["mode"], json!("canvas"));
    let placed: Vec<&str> = saved["items"]
        .as_array()
        .expect("items")
        .iter()
        .filter_map(|item| item["breakpoint"].as_str())
        .collect();
    assert_eq!(placed, ["desktop", "tablet", "mobile"], "every view placed");
    assert!(
        saved.get("camera").is_none(),
        "the camera stays in the session"
    );
    assert!(preset(&file(&dir, ConfigFile::Project), "my board").is_null());
    let shell = core.host().shell_state();
    assert_eq!(shell.preset.as_deref(), Some("my board"));
    assert_eq!(shell.mode, LayoutMode::Canvas);
    assert!(
        core.host()
            .config()
            .layouts
            .presets
            .contains_key("my board")
    );
}

#[tokio::test]
async fn the_split_keeps_its_ratio_and_sides() {
    let (dir, setup) = project();
    let core = &setup.core;
    ok(
        core,
        MODE_SPLIT,
        json!({ "ratio": 0.3, "primary": ["mobile"], "secondary": ["desktop", "tablet"] }),
    );
    let answer = ok(core, SAVE_PRESET, json!({ "id": "side_by-side.2" }));
    assert_eq!(answer["mode"], json!("split"));
    let local = file(&dir, ConfigFile::Local);
    let saved = preset(&local, "side_by-side.2");
    assert_eq!(saved["mode"], json!("split"));
    assert_eq!(saved["ratio"], json!(0.3));
    assert_eq!(saved["primary"], json!(["mobile"]));
    assert_eq!(saved["secondary"], json!(["desktop", "tablet"]));
    assert_eq!(
        core.host().shell_state().preset.as_deref(),
        Some("side_by-side.2")
    );
}

#[tokio::test]
async fn the_grid_goes_to_the_project_file_when_asked() {
    let (dir, setup) = project();
    let core = &setup.core;
    let answer = ok(
        core,
        SAVE_PRESET,
        json!({ "id": "team", "file": "project" }),
    );
    assert_eq!(
        answer,
        json!({ "id": "team", "file": "project", "mode": "grid" })
    );
    let project = file(&dir, ConfigFile::Project);
    let saved = preset(&project, "team");
    assert_eq!(saved["mode"], json!("grid"));
    let cells = saved["layout"]["cells"]
        .as_array()
        .expect("the grid's cells");
    assert_eq!(cells.len(), 3);
    assert!(saved["layout"].get("modes").is_none());
    assert!(
        preset(&project, "board").is_object(),
        "the other presets stay"
    );
    assert!(preset(&file(&dir, ConfigFile::Local), "team").is_null());
    assert_eq!(core.host().shell_state().preset.as_deref(), Some("team"));
}

#[tokio::test]
async fn the_user_setting_names_the_default_file() {
    let (dir, setup) = project();
    let core = &setup.core;
    let mut user = core.user_settings();
    user.layout_modes.save_preset_to = rb_config::PresetFile::Project;
    core.replace_user_settings(user);
    ok(core, SAVE_PRESET, json!({ "id": "shared" }));
    assert!(preset(&file(&dir, ConfigFile::Project), "shared").is_object());
}

#[tokio::test]
async fn saving_again_replaces_the_preset_of_that_file() {
    let (dir, setup) = project();
    let core = &setup.core;
    ok(core, MODE_CANVAS, Value::Null);
    ok(core, SAVE_PRESET, json!({ "id": "mine" }));
    ok(core, MODE_SPLIT, json!({ "ratio": 0.4 }));
    ok(core, SAVE_PRESET, json!({ "id": "mine" }));
    let local = file(&dir, ConfigFile::Local);
    let saved = preset(&local, "mine");
    assert_eq!(saved["mode"], json!("split"));
    assert!(saved.get("items").is_none(), "{saved}");
    assert_eq!(saved["ratio"], json!(0.4));
}

#[tokio::test]
async fn focus_mode_keeps_the_breakpoint_shown() {
    let (dir, setup) = project();
    let core = &setup.core;
    crate::presets::run_command(
        core,
        "layout.focus_mode",
        &json!({ "enabled": true, "focus": "tablet" }),
    )
    .expect("a presets command")
    .expect("focus mode");
    ok(core, SAVE_PRESET, json!({ "id": "one" }));
    let saved = preset(&file(&dir, ConfigFile::Local), "one").clone();
    assert_eq!(saved["mode"], json!("focus"));
    assert_eq!(saved["focus"], json!("tablet"));
}

#[tokio::test]
async fn a_bad_name_is_refused_and_nothing_is_written() {
    let (dir, setup) = project();
    let core = &setup.core;
    let before = file(&dir, ConfigFile::Project);
    let long = "x".repeat(MAX_ID_CHARS + 1);
    for id in ["", "   ", "a/b", "quote\"d", "tab\there", long.as_str()] {
        let refused = run(core, SAVE_PRESET, &json!({ "id": id }));
        assert!(
            matches!(refused, Err(CommandError::InvalidArgs(_))),
            "{id:?}: {refused:?}"
        );
    }
    let unknown = run(core, SAVE_PRESET, &json!({ "id": "a", "file": "team" }));
    assert!(matches!(unknown, Err(CommandError::InvalidArgs(_))));
    let extra = run(core, SAVE_PRESET, &json!({ "id": "a", "name": "b" }));
    assert!(matches!(extra, Err(CommandError::InvalidArgs(_))));
    assert_eq!(file(&dir, ConfigFile::Project), before);
    assert!(file(&dir, ConfigFile::Local).is_null());
    assert_eq!(core.host().shell_state().preset, None);
}

#[test]
fn the_longest_name_and_its_characters_pass() {
    let longest = "a".repeat(MAX_ID_CHARS);
    assert_eq!(preset_id(&longest).expect("40 characters"), longest);
    assert_eq!(
        preset_id(" Board 2.1_x-y ").expect("trimmed"),
        "Board 2.1_x-y"
    );
}

#[tokio::test]
async fn without_a_name_the_shell_asks_for_one() {
    let (dir, setup) = project();
    let core = &setup.core;
    assert_eq!(ok(core, SAVE_PRESET, Value::Null), Value::Null);
    assert_eq!(ok(core, SAVE_PRESET, json!({})), Value::Null);
    let calls = setup.ui.calls();
    assert!(calls.contains(&UiCall::FocusShell));
    assert!(calls.iter().any(|call| matches!(
        call,
        UiCall::Shell(script) if script.contains("uiCommand") && script.contains(SAVE_PRESET)
    )));
    assert!(file(&dir, ConfigFile::Local).is_null());
}
