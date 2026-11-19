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
/// `on_start` returns before anything is spawned. On by default since RB-090-14.
#[test]
fn the_gate_opens_only_with_the_feature_and_the_setting() {
    let mut settings = UserSettings::default();
    assert!(gate_open(true, &settings), "on by default");
    assert!(!gate_open(false, &settings), "a build without the feature");
    settings.updates.enabled = false;
    assert!(!gate_open(true, &settings));
}

/// RB-090-14: the default build carries self-update.
#[test]
fn the_default_build_carries_self_update() {
    assert_eq!(SELF_UPDATE_BUILT, cfg!(feature = "self-update"));
    assert_eq!(is_locked("updates.enabled"), !SELF_UPDATE_BUILT);
}

/// The CEF notice is silent while updates come by themselves, and speaks with the check off.
#[test]
fn only_updates_checked_by_themselves_silence_the_cef_notice() {
    let mut settings = UserSettings::default();
    assert!(checks_by_itself(true, &settings));
    assert!(!checks_by_itself(false, &settings));
    settings.updates.check = UpdateCheck::Off;
    assert!(!checks_by_itself(true, &settings));
    settings.updates.check = UpdateCheck::OnStart;
    settings.updates.enabled = false;
    assert!(!checks_by_itself(true, &settings));
}

#[test]
fn the_commands_sit_in_the_settings_group_and_are_covered_by_their_tools() {
    let defs = definitions().expect("definitions");
    let rows: Vec<(String, String)> = defs
        .iter()
        .map(|def| {
            let binding = def.default_bindings[0].to_string();
            (def.id.as_str().to_owned(), binding)
        })
        .collect();
    assert_eq!(
        rows,
        [
            (CHECK_COMMAND.to_owned(), "Ctrl+Shift+Space , U".to_owned()),
            (
                INSTALL_COMMAND.to_owned(),
                "Ctrl+Shift+Space , I".to_owned()
            ),
        ]
    );
    for def in &defs {
        assert_eq!(def.group, Group::Settings);
        assert!(matches!(&def.mcp, McpCoverage::Covered(tool) if mcp::is_tool(tool)));
    }
}

/// MCP never runs the install: its tool asks the person, whose click runs the command.
#[test]
fn mcp_cannot_run_the_install_command() {
    assert!(refuse_mcp(INSTALL_COMMAND).is_err());
    assert!(refuse_mcp(CHECK_COMMAND).is_ok());
    assert!(refuse_mcp("app.about").is_ok());
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

/// RB-090-14 review: a version switched at quit proves itself at its start even with updates
/// switched off since, or its second start would take it back; the check needs both settings.
#[test]
fn the_start_proves_a_switched_update_whatever_the_settings_and_checks_only_when_asked() {
    let mut settings = UserSettings::default();
    assert_eq!(start_work(true, &settings), (true, true));
    settings.updates.check = UpdateCheck::Off;
    assert_eq!(start_work(true, &settings), (true, false));
    settings.updates.check = UpdateCheck::OnStart;
    settings.updates.enabled = false;
    assert_eq!(start_work(true, &settings), (true, false));
    assert_eq!(start_work(false, &settings), (false, false));
}

/// Settings > About names the real state (RB-091-05), not "disabled in this build".
#[test]
fn about_names_how_this_installation_updates() {
    use rb_ipc::AboutUpdates;
    let mut settings = UserSettings::default();
    assert_eq!(about_state(false, &settings), AboutUpdates::NotBuilt);
    assert_eq!(about_state(true, &settings), AboutUpdates::OnStart);
    settings.updates.check = UpdateCheck::Off;
    assert_eq!(about_state(true, &settings), AboutUpdates::Manual);
    settings.updates.enabled = false;
    assert_eq!(about_state(true, &settings), AboutUpdates::Off);
    assert_eq!(about_state(false, &settings), AboutUpdates::NotBuilt);
}
