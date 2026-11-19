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

#[test]
fn grid_drops_a_default_preset_of_another_mode() {
    let mut config = json!({
        "layouts": { "default": "board", "presets": { "board": { "mode": "canvas" } } }
    });
    apply(&mut config, Some(SetupLayout::Grid), None);
    assert!(config["layouts"].get("default").is_none());
    assert_eq!(config["layouts"]["presets"]["board"]["mode"], "canvas");
    // A grid preset as the default stays.
    let mut grid = json!({ "layouts": { "default": "two", "presets": { "two": {} } } });
    apply(&mut grid, Some(SetupLayout::Grid), None);
    assert_eq!(grid["layouts"]["default"], "two");
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
