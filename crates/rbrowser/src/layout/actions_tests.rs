use rb_config::{LayoutMode, SplitOrientation};
use serde_json::json;

use super::*;
use crate::layout::test_support::{config, setup, setup_in, view_ids};

fn run_ok(core: &Core, id: &str, args: Value) -> Value {
    run(core, id, &args).unwrap_or_else(|error| panic!("{id}: {error}"))
}

fn select(core: &Core, preset: &str) {
    crate::presets::run_command(core, "layout.select_preset", &json!({ "id": preset }))
        .expect("a presets command")
        .expect("selects");
}

/// Grid -> horizontal -> split -> canvas -> grid: the same views throughout (their browsers and
/// page state live on), and the shell state follows.
#[tokio::test]
async fn switching_modes_keeps_every_view() {
    let setup = setup(config());
    let core = &setup.core;
    let before = view_ids(&setup);
    assert_eq!(before.len(), 3);
    for (id, mode) in [
        (MODE_HORIZONTAL, LayoutMode::Horizontal),
        (MODE_SPLIT, LayoutMode::Split),
        (MODE_CANVAS, LayoutMode::Canvas),
        (MODE_GRID, LayoutMode::Grid),
    ] {
        run_ok(core, id, Value::Null);
        assert_eq!(core.host().shell_state().mode, mode, "{id}");
        assert_eq!(view_ids(&setup), before, "{id}: no view closed or opened");
    }
    assert!(core.host().mode_spec().is_none());
    assert!(setup.ui.last_grid().expect("a grid").placed.is_none());
}

#[tokio::test]
async fn the_divider_moves_by_the_keyboard_step_and_stops_at_the_ends() {
    let setup = setup(config());
    let core = &setup.core;
    run_ok(core, MODE_SPLIT, Value::Null);
    let answer = run_ok(core, SPLIT_GROW, Value::Null);
    assert_eq!(answer["ratio"], json!(0.55));
    assert_eq!(
        answer["persisted"],
        Value::Null,
        "no project: the session keeps it"
    );
    run_ok(core, SPLIT_SHRINK, Value::Null);
    for _ in 0..20 {
        run_ok(core, SPLIT_SHRINK, Value::Null);
    }
    assert!(matches!(
        core.host().mode_spec().map(|spec| spec.kind),
        Some(ModeKind::Split { ratio, .. }) if ratio == 0.1
    ));
    assert_eq!(core.host().shell_state().modes.fields.ratio, Some(0.1));
}

#[tokio::test]
async fn a_split_preset_shows_its_sides_and_swap_trades_them() {
    let setup = setup(config());
    let core = &setup.core;
    select(core, "pair");
    let spec = core.host().mode_spec().expect("split");
    assert_eq!(spec.breakpoints, ["desktop", "tablet", "mobile"]);
    assert!(matches!(spec.kind, ModeKind::Split { ratio, primary: 1, .. } if ratio == 0.6));
    run_ok(core, SPLIT_SWAP, Value::Null);
    let spec = core.host().mode_spec().expect("split");
    assert_eq!(spec.breakpoints, ["tablet", "mobile", "desktop"]);
    assert!(matches!(spec.kind, ModeKind::Split { primary: 2, .. }));
}

#[tokio::test]
async fn mode_arguments_set_the_fields() {
    let setup = setup(config());
    let core = &setup.core;
    run_ok(
        core,
        MODE_SPLIT,
        json!({ "ratio": 0.3, "orientation": "horizontal", "primary": ["mobile"] }),
    );
    let spec = core.host().mode_spec().expect("split");
    assert_eq!(spec.breakpoints[0], "mobile");
    assert_eq!(
        spec.kind,
        ModeKind::Split {
            orientation: SplitOrientation::Horizontal,
            ratio: 0.3,
            primary: 1
        }
    );
    assert!(matches!(
        run(core, MODE_SPLIT, &json!({ "ratio": "half" })),
        Err(CommandError::InvalidArgs(_))
    ));
}

#[tokio::test]
async fn split_commands_outside_split_mode_fail_with_their_key() {
    let setup = setup(config());
    let core = &setup.core;
    for id in [SPLIT_GROW, SPLIT_SHRINK, SPLIT_SWAP] {
        match run(core, id, &Value::Null) {
            Err(CommandError::Failed(key)) => assert_eq!(key, "layout.error.not_split"),
            other => panic!("{id}: {other:?}"),
        }
    }
}

#[tokio::test]
async fn the_minimap_switch_flips_for_the_session() {
    let setup = setup(config());
    let core = &setup.core;
    assert_eq!(
        run_ok(core, MINIMAP_TOGGLE, Value::Null)["minimap"],
        json!(false)
    );
    assert_eq!(core.host().shell_state().modes.minimap, Some(false));
    assert_eq!(
        run_ok(core, MINIMAP_TOGGLE, Value::Null)["minimap"],
        json!(true)
    );
}

#[tokio::test]
async fn focus_mode_returns_to_the_presets_mode() {
    let setup = setup(config());
    let core = &setup.core;
    select(core, "pair");
    let toggle = |core: &Core| {
        crate::presets::run_command(core, "layout.focus_mode", &Value::Null)
            .expect("a presets command")
            .expect("toggles");
    };
    toggle(core);
    assert_eq!(core.host().shell_state().mode, LayoutMode::Focus);
    toggle(core);
    assert_eq!(core.host().shell_state().mode, LayoutMode::Split);
    assert_eq!(core.host().shell_state().preset.as_deref(), Some("pair"));
}

/// The ratio is written into the preset's file, so it is there after a restart.
#[tokio::test]
async fn the_ratio_is_written_into_the_preset() {
    let dir = tempfile::tempdir().expect("a temporary folder");
    let root = dir.path().to_path_buf();
    std::fs::create_dir_all(root.join(".rbrowser")).expect("the config folder");
    let value = serde_json::to_value(config()).expect("serializes");
    rb_config::write_config_file(&root, rb_config::ConfigFile::Project, &value)
        .expect("the project file");
    let setup = setup_in(config(), Some(root.clone()));
    let core = &setup.core;
    select(core, "pair");
    let answer = run_ok(core, SPLIT_GROW, Value::Null);
    assert_eq!(answer["persisted"], json!("pair"));
    let written =
        rb_config::read_json(&rb_config::ConfigFile::Project.path(&root)).expect("reads back");
    assert_eq!(written["layouts"]["presets"]["pair"]["ratio"], json!(0.65));
    assert_eq!(
        written["layouts"]["presets"]["pair"]["mode"],
        json!("split")
    );
    assert!(!rb_config::ConfigFile::Local.path(&root).exists());

    // A mode switched on a preset of another mode keeps its fields in the top-level layout's
    // `layout.modes` of the local file; no preset is made and the current one stays.
    run_ok(core, MODE_HORIZONTAL, json!({ "gap": 12 }));
    let local =
        rb_config::read_json(&rb_config::ConfigFile::Local.path(&root)).expect("the local file");
    assert_eq!(local["layout"]["modes"]["horizontal"], json!({ "gap": 12 }));
    assert!(local.get("layouts").is_none(), "{local}");
    assert_eq!(core.host().shell_state().preset.as_deref(), Some("pair"));
}

#[test]
fn merge_takes_only_what_the_patch_sets() {
    let mut fields = ModeFields {
        ratio: Some(0.4),
        primary: vec!["a".into()],
        ..ModeFields::default()
    };
    merge(
        &mut fields,
        &ModeFields {
            snap: Some(4),
            ..ModeFields::default()
        },
    );
    assert_eq!(fields.ratio, Some(0.4));
    assert_eq!(fields.primary, ["a"]);
    assert_eq!(fields.snap, Some(4));
}

#[test]
fn every_field_is_written_or_removed() {
    let map = field_map(&ModeFields {
        ratio: Some(0.5),
        ..ModeFields::default()
    });
    assert_eq!(map.len(), 8);
    assert_eq!(map["ratio"], json!(0.5));
    assert_eq!(map["items"], Value::Null);
}
