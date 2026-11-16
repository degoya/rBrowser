use serde_json::json;

use super::*;

#[test]
fn the_commands_are_in_the_capture_group_with_free_chords_and_no_tool() {
    let defs = definitions().expect("valid ids");
    let ids: Vec<&str> = defs.iter().map(|def| def.id.as_str()).collect();
    assert_eq!(ids, [TOGGLE_HIDE, TOGGLE_FREEZE, PREVIEW]);
    for def in &defs {
        assert_eq!(def.group, Group::Capture);
        assert_eq!(def.title_key, format!("commands.{}", def.id.as_str()));
        assert!(
            matches!(def.mcp, McpCoverage::Omitted(ref reason) if reason.starts_with("ui-only"))
        );
        assert_eq!(def.default_bindings.len(), 1);
    }
    // `C F` is capture.full_page, `C B` visual.baseline_update.
    let chords: Vec<[char; 2]> = ROWS.iter().map(|(_, keys, _)| *keys).collect();
    assert_eq!(chords, [['C', 'H'], ['C', 'N'], ['C', 'P']]);
}

#[test]
fn enabled_is_a_boolean_or_a_flip() {
    assert_eq!(enabled(&json!({})).expect("flip"), None);
    assert_eq!(enabled(&Value::Null).expect("flip"), None);
    assert_eq!(
        enabled(&json!({ "enabled": false })).expect("off"),
        Some(false)
    );
    assert!(enabled(&json!({ "enabled": "on" })).is_err());
}

#[test]
fn a_toggle_flips_the_value_in_effect_and_keeps_the_other() {
    let session = CleanOverrides {
        hide_banners: None,
        freeze_animations: Some(false),
    };
    let (next, on) = toggled(session, true, true, None);
    assert!(!on);
    assert_eq!(next.hide_banners, Some(false));
    assert_eq!(next.freeze_animations, Some(false), "kept");
    let (next, on) = toggled(next, false, false, None);
    assert!(on);
    assert_eq!(next.freeze_animations, Some(true));
    let (next, on) = toggled(next, false, true, Some(true));
    assert!(on, "enabled wins over the flip");
    assert_eq!(next.freeze_animations, Some(true));
}
