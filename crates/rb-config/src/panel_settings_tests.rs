use super::*;
use serde_json::json;

#[test]
fn the_default_is_the_icon_rail_docked_right() {
    let settings: PanelSettings = serde_json::from_value(json!({})).expect("defaults");
    assert_eq!(settings, PanelSettings::default());
    assert_eq!(settings.dock, PanelDock::Right);
    assert_eq!(settings.dock_state, PanelDockState::Rail);
    assert_eq!(settings.dock_at_start, PanelDockAtStart::Rail);
    assert_eq!(settings.close_to, PanelCloseTo::Rail);
    assert_eq!(
        settings.dock_size,
        PanelDockSize {
            left: 520,
            right: 520,
            bottom: 320
        }
    );
}

#[test]
fn the_positions_are_written_in_lower_case() {
    for (text, dock) in [
        ("floating", PanelDock::Floating),
        ("left", PanelDock::Left),
        ("right", PanelDock::Right),
        ("bottom", PanelDock::Bottom),
    ] {
        assert_eq!(serde_json::to_value(dock).expect("writes"), json!(text));
        let read: PanelDock = serde_json::from_value(json!(text)).expect("reads");
        assert_eq!(read, dock);
    }
    assert!(serde_json::from_value::<PanelDock>(json!("top")).is_err());
}

#[test]
fn a_size_is_kept_per_side() {
    let settings: PanelSettings = serde_json::from_value(json!({
        "dock": "left", "dockSize": { "left": 300, "bottom": 200 }
    }))
    .expect("reads");
    assert_eq!(settings.dock, PanelDock::Left);
    assert_eq!(
        (
            settings.dock_size.left,
            settings.dock_size.right,
            settings.dock_size.bottom
        ),
        (300, 520, 200)
    );
}

#[test]
fn the_user_settings_carry_it_under_panels() {
    let user: crate::UserSettings =
        serde_json::from_value(json!({ "panels": { "dock": "bottom" } })).expect("reads");
    assert_eq!(user.panels.dock, PanelDock::Bottom);
    assert_eq!(user.panels.dock_size.bottom, 320);
    assert_eq!(crate::UserSettings::default().panels.dock, PanelDock::Right);
}

#[test]
fn the_side_bar_starts_with_titles() {
    let settings = PanelSettings::default();
    assert_eq!(settings.sidebar, PanelSidebar::Expanded);
    let read: PanelSettings = serde_json::from_value(json!({ "sidebar": "icons" })).expect("reads");
    assert_eq!(read.sidebar, PanelSidebar::Icons);
    assert_eq!(
        serde_json::to_value(read).expect("writes")["sidebar"],
        json!("icons")
    );
    assert!(serde_json::from_value::<PanelSidebar>(json!("hidden")).is_err());
}

#[test]
fn the_new_keys_read_and_write_in_lower_case() {
    let read: PanelSettings = serde_json::from_value(json!({
        "dockState": "hidden", "dockAtStart": "last", "closeTo": "hidden"
    }))
    .expect("reads");
    assert_eq!(
        (read.dock_state, read.dock_at_start, read.close_to),
        (
            PanelDockState::Hidden,
            PanelDockAtStart::Last,
            PanelCloseTo::Hidden
        )
    );
    let written = serde_json::to_value(read).expect("writes");
    assert_eq!(written["dockState"], json!("hidden"));
    assert_eq!(written["dockAtStart"], json!("last"));
    assert_eq!(written["closeTo"], json!("hidden"));
    for bad in [
        json!({ "dockState": "folded" }),
        json!({ "dockAtStart": "open" }),
        json!({ "closeTo": "nothing" }),
    ] {
        assert!(
            serde_json::from_value::<PanelSettings>(bad.clone()).is_err(),
            "{bad}"
        );
    }
}

/// RB-080-93: `panels.dockCollapsed` (RB-080-61) is read once more - `true` the rail, `false`
/// open - and never written again; `dockState`, when the file has both, wins.
#[test]
fn the_old_dock_collapsed_is_migrated_to_the_dock_state() {
    for (old, state) in [(true, PanelDockState::Rail), (false, PanelDockState::Open)] {
        let read: PanelSettings =
            serde_json::from_value(json!({ "dockCollapsed": old })).expect("reads");
        assert_eq!(read.dock_state, state, "dockCollapsed {old}");
        let written = serde_json::to_value(read).expect("writes");
        assert!(written.get("dockCollapsed").is_none(), "{written}");
    }
    let both: PanelSettings =
        serde_json::from_value(json!({ "dockCollapsed": true, "dockState": "open" }))
            .expect("reads");
    assert_eq!(both.dock_state, PanelDockState::Open);
    assert!(serde_json::from_value::<PanelSettings>(json!({ "dockCollapsed": "yes" })).is_err());
}

#[test]
fn the_schema_knows_the_new_keys_and_not_the_old_one() {
    let schema = serde_json::to_value(schemars::schema_for!(PanelSettings)).expect("schema");
    let properties = &schema["properties"];
    for key in [
        "dock",
        "dockSize",
        "dockState",
        "sidebar",
        "dockAtStart",
        "closeTo",
    ] {
        assert!(properties.get(key).is_some(), "{key}");
    }
    assert!(properties.get("dockCollapsed").is_none());
    assert_eq!(properties["dockState"]["default"], json!("rail"));
    assert_eq!(properties["dock"]["default"], json!("right"));
}

#[test]
fn three_hidden_panel_frames_stay_loaded_by_default() {
    // RB-090-38: switching back to one of the last panels shown does not load it again.
    let settings: PanelSettings = serde_json::from_value(json!({})).expect("defaults");
    assert_eq!(settings.keep_frames, 3);
    assert_eq!(PanelSettings::default().keep_frames, 3);
    let none: PanelSettings =
        serde_json::from_value(json!({ "keepFrames": 0 })).expect("keep none");
    assert_eq!(none.keep_frames, 0);
    assert_eq!(serde_json::to_value(none).expect("writes")["keepFrames"], 0);
}
