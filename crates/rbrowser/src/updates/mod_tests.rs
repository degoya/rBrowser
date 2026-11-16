use super::*;

#[test]
fn without_the_feature_every_update_key_but_the_cef_notice_is_locked() {
    for key in LOCKED_KEYS {
        assert!(locked_in(false, key), "{key}");
        assert!(!locked_in(true, key), "{key}");
    }
    assert!(!locked_in(false, "updates.cefAgeWarning"));
    assert!(!locked_in(false, "ui.theme"));
}

/// No network call at all while the feature or the setting is off: the gate stays shut, and
/// `on_start` returns before anything is spawned.
#[test]
fn the_gate_opens_only_with_the_feature_and_the_setting() {
    let mut settings = UserSettings::default();
    assert!(!gate_open(true, &settings), "off by default");
    assert!(!gate_open(false, &settings));
    settings.updates.enabled = true;
    assert!(!gate_open(false, &settings), "a build without the feature");
    assert!(gate_open(true, &settings));
}

/// Phase 1 builds without the feature.
#[test]
fn this_build_carries_no_self_update_unless_asked() {
    assert_eq!(SELF_UPDATE_BUILT, cfg!(feature = "self-update"));
    assert_eq!(is_locked("updates.enabled"), !SELF_UPDATE_BUILT);
}

#[test]
fn every_locked_key_is_a_setting_with_a_text() {
    let (registry, issues) = crate::control::settings::settings_registry();
    assert!(issues.is_empty(), "{issues:?}");
    for key in LOCKED_KEYS {
        let def = registry.get(key).expect("a setting");
        assert!(def.protected_from_mcp, "{key} is MCP-protected");
    }
    assert!(rb_i18n::is_well_formed_key(INACTIVE_KEY));
}
