use rb_commands::lint::lint_defaults;
use rb_commands::{Platform, builtin, builtin_reserved};

use super::*;

/// Every core command the app registers besides this module's.
fn other_core_defs() -> Vec<CommandDef> {
    let mut defs = builtin::definitions();
    defs.extend(crate::commands::definitions().expect("the grid's definitions"));
    defs.extend(crate::sessions::defs::definitions().expect("sessions"));
    defs.extend(crate::breakpoint_keys::definitions().expect("breakpoint keys"));
    defs.extend(crate::bookmarks::definitions().expect("bookmarks"));
    defs.extend(crate::flows::definitions().expect("flows"));
    defs.push(rb_diag::command_def().expect("diagnostics.create"));
    defs.extend(rb_plugin_host::core_command_defs().expect("plugins.*"));
    defs
}

#[test]
fn the_defaults_pass_the_lint_against_the_reserved_list_and_the_others() {
    let mut defs = other_core_defs();
    defs.extend(definitions().expect("definitions"));
    let issues: Vec<String> = lint_defaults(&defs, builtin_reserved(), Platform::Windows)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(issues.is_empty(), "shortcut lint:\n{}", issues.join("\n"));
}

#[test]
fn both_commands_have_their_chords_and_no_tool() {
    let defs = definitions().expect("definitions");
    let summary: Vec<(String, Vec<String>, McpCoverage)> = defs
        .iter()
        .map(|def| {
            (
                def.id.as_str().to_owned(),
                def.default_bindings
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                def.mcp.clone(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            (
                VITALS.to_owned(),
                vec!["Ctrl+Shift+Space D C".to_owned()],
                McpCoverage::Omitted(OMITTED.to_owned())
            ),
            (
                RESOURCES.to_owned(),
                vec!["Ctrl+Shift+Space D R".to_owned()],
                McpCoverage::Omitted(OMITTED.to_owned())
            ),
        ]
    );
    assert!(defs.iter().all(|def| def.group == Group::Devtools));
}

#[test]
fn each_command_switches_its_setting() {
    assert_eq!(setting_of(VITALS), Some("vitals.enabled"));
    assert_eq!(setting_of(RESOURCES), Some("resources.enabled"));
    assert_eq!(setting_of("debug.other"), None);
}

fn core() -> Arc<Core> {
    use crate::cdp::fake::FakeBrowser;
    use crate::control::CoreSetup;
    use crate::host::Host;
    use crate::host::state::default_config;
    use crate::host::ui::recording::RecordingUi;
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, tokio::runtime::Handle::current());
    host.open_config(default_config(), None).expect("opens");
    Core::new(
        host,
        ui,
        tokio::runtime::Handle::current(),
        CoreSetup::default(),
    )
}

/// `Leader D C` and `Leader D R` switch a display that may show nothing yet (no report in, no
/// Chromium view), so each switch names its new state in the status bar (RB-080-18).
#[tokio::test]
async fn a_switch_names_its_new_state_in_the_status_bar() {
    let core = core();
    assert!(core.user_settings().vitals.enabled);
    for (id, expected, key, setting) in [
        (VITALS, false, VITALS_OFF_KEY, "vitals.enabled"),
        (VITALS, true, VITALS_ON_KEY, "vitals.enabled"),
        (RESOURCES, false, RESOURCES_OFF_KEY, "resources.enabled"),
        (RESOURCES, true, RESOURCES_ON_KEY, "resources.enabled"),
    ] {
        let answer = toggle(&core, id).expect("switched");
        assert_eq!(answer["enabled"], json!(expected), "{id}");
        let items = core.status_log().items();
        let last = items.last().expect("a status message");
        assert_eq!(last.message_key, key);
        assert_eq!(last.level, rb_ipc::NotificationLevel::Info);
        assert_eq!(last.settings_key.as_deref(), Some(setting));
    }
}
