use super::*;
use rb_config::{LayoutPreset, ModeFields};
use serde_json::json;

fn config(presets: &[(&str, LayoutMode)]) -> ProjectConfig {
    let mut config = ProjectConfig::default();
    for (id, mode) in presets {
        config.layouts.presets.insert(
            (*id).to_owned(),
            LayoutPreset {
                mode: *mode,
                ..LayoutPreset::default()
            },
        );
    }
    config
}

#[test]
fn the_active_preset_of_the_mode_takes_the_fields() {
    let config = config(&[("board", LayoutMode::Canvas)]);
    assert_eq!(
        target(&config, Some("board"), LayoutMode::Canvas),
        Target::Preset("board".to_owned())
    );
}

/// No preset named after a mode any more: the top-level layout keeps the fields.
#[test]
fn another_mode_goes_to_the_top_level_layout() {
    let grids = config(&[("main", LayoutMode::Grid), ("split", LayoutMode::Grid)]);
    assert_eq!(
        target(&grids, Some("main"), LayoutMode::Canvas),
        Target::TopLevel
    );
    assert_eq!(target(&grids, None, LayoutMode::Split), Target::TopLevel);
    assert_eq!(Target::TopLevel.label(), "layout");
}

#[test]
fn stored_reads_the_preset_or_the_top_level_modes() {
    let mut config = config(&[("pair", LayoutMode::Split)]);
    config.layouts.presets["pair"].modes.ratio = Some(0.6);
    config.layout.modes.set(
        LayoutMode::Split,
        ModeFields {
            ratio: Some(0.25),
            ..ModeFields::default()
        },
    );
    assert_eq!(
        stored(&config, Some("pair"), LayoutMode::Split).ratio,
        Some(0.6)
    );
    assert_eq!(stored(&config, None, LayoutMode::Split).ratio, Some(0.25));
    assert_eq!(
        stored(&config, None, LayoutMode::Canvas),
        ModeFields::default()
    );
}

#[test]
fn the_top_level_fields_go_under_layout_modes_without_a_mode_key() {
    let mut document = json!({ "layout": { "columns": 4 } });
    let mut fields = Map::new();
    fields.insert("ratio".to_owned(), json!(0.25));
    let path = Target::TopLevel.path(LayoutMode::Split);
    patch(&mut document, &path, None, &fields);
    assert_eq!(
        document,
        json!({ "layout": { "columns": 4, "modes": { "split": { "ratio": 0.25 } } } })
    );
    assert!(at(&document, &path).is_some());
    assert!(at(&document, &Target::TopLevel.path(LayoutMode::Canvas)).is_none());
}

#[test]
fn patch_creates_the_way_sets_the_mode_and_removes_nulls() {
    let mut document = json!({ "version": 1 });
    let split = Target::Preset("split".to_owned()).path(LayoutMode::Split);
    let mut fields = Map::new();
    fields.insert("ratio".to_owned(), json!(0.6));
    fields.insert("primary".to_owned(), json!(["desktop"]));
    patch(&mut document, &split, Some(LayoutMode::Split), &fields);
    assert_eq!(
        document,
        json!({ "version": 1, "layouts": { "presets": { "split": {
            "mode": "split", "ratio": 0.6, "primary": ["desktop"]
        } } } })
    );
    let mut removal = Map::new();
    removal.insert("primary".to_owned(), Value::Null);
    patch(&mut document, &split, Some(LayoutMode::Split), &removal);
    assert_eq!(
        document["layouts"]["presets"]["split"],
        json!({ "mode": "split", "ratio": 0.6 })
    );
}

#[test]
fn patch_keeps_the_other_keys_of_the_preset() {
    let mut document = json!({ "layouts": { "default": "board", "presets": {
        "board": { "mode": "canvas", "snap": 16, "layout": { "columns": 1 } }
    } } });
    let mut fields = Map::new();
    fields.insert(
        "items".to_owned(),
        json!([{ "breakpoint": "a", "x": 8, "y": 16 }]),
    );
    let board = Target::Preset("board".to_owned()).path(LayoutMode::Canvas);
    patch(&mut document, &board, Some(LayoutMode::Canvas), &fields);
    let board = &document["layouts"]["presets"]["board"];
    assert_eq!(board["snap"], json!(16));
    assert_eq!(board["layout"], json!({ "columns": 1 }));
    assert_eq!(board["items"][0]["x"], json!(8));
    assert_eq!(document["layouts"]["default"], json!("board"));
}
