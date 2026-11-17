use rb_commands::lint::lint_defaults;
use rb_commands::{Platform, builtin, builtin_reserved};
use serde_json::json;

use super::*;
use crate::host::ui::recording::UiCall;
use crate::layout::test_support::{config, setup};

/// The core commands the app registers besides this module's, the View group's included.
fn other_core_defs() -> Vec<CommandDef> {
    let mut defs = builtin::definitions();
    defs.extend(crate::commands::definitions().expect("the grid's definitions"));
    defs.extend(crate::sessions::defs::definitions().expect("sessions"));
    defs.extend(crate::breakpoint_keys::definitions().expect("breakpoint keys"));
    defs.extend(crate::bookmarks::definitions().expect("bookmarks"));
    defs.extend(crate::flows::definitions().expect("flows"));
    defs.extend(crate::vitals::defs::definitions().expect("vitals"));
    defs.extend(crate::status::defs::definitions().expect("status"));
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
fn both_commands_sit_in_the_view_group_without_a_tool_of_their_own() {
    let defs = definitions().expect("definitions");
    let summary: Vec<(String, String, Vec<String>, McpCoverage)> = defs
        .iter()
        .map(|def| {
            (
                def.id.as_str().to_owned(),
                def.title_key.clone(),
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
                CYCLE.to_owned(),
                "commands.ui.theme.cycle".to_owned(),
                vec!["Ctrl+Shift+Space V 0".to_owned()],
                McpCoverage::Omitted(OMITTED.to_owned())
            ),
            (
                SET.to_owned(),
                "commands.ui.theme.set".to_owned(),
                vec!["Ctrl+Shift+Space V =".to_owned()],
                McpCoverage::Omitted(OMITTED.to_owned())
            ),
        ]
    );
    assert!(defs.iter().all(|def| def.group == Group::View));
}

/// The plugins' chords under `V` are theirs: `0` and `=` are none of them.
#[test]
fn the_chords_are_free_among_the_bundled_plugins() {
    let plugins = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let mut manifests = 0;
    for entry in std::fs::read_dir(plugins).expect("plugins/") {
        let manifest = entry.expect("entry").path().join("manifest.toml");
        let Ok(text) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        manifests += 1;
        for chord in ["\"Ctrl+Shift+Space V 0\"", "\"Ctrl+Shift+Space V =\""] {
            assert!(
                !text.contains(chord),
                "{} binds {chord}",
                manifest.display()
            );
        }
    }
    assert!(manifests > 10, "the plugins were read: {manifests}");
}

#[tokio::test]
async fn cycle_steps_the_setting_and_the_window_and_the_status_bar_follow() {
    let setup = setup(config());
    for expected in [Theme::Light, Theme::Dark, Theme::System] {
        let answer = run(&setup.core, CYCLE, &Value::Null).expect("cycles");
        assert_eq!(answer, json!({ "theme": word(expected) }));
        assert_eq!(setup.core.user_settings().ui.theme, expected);
        assert!(setup.ui.calls().contains(&UiCall::AppTheme(expected)));
        let messages = setup.core.status_log().items();
        let last = messages.last().expect("a notice");
        assert_eq!(last.message_key, notice_key(expected));
        assert_eq!(last.settings_key.as_deref(), Some(THEME_KEY));
    }
}

#[tokio::test]
async fn set_takes_a_theme_and_without_one_follows_windows_again() {
    let setup = setup(config());
    run(&setup.core, SET, &json!({ "theme": "dark" })).expect("sets");
    assert_eq!(setup.core.user_settings().ui.theme, Theme::Dark);
    run(&setup.core, SET, &json!({})).expect("sets");
    assert_eq!(setup.core.user_settings().ui.theme, Theme::System);
    run(&setup.core, SET, &json!({ "theme": "light" })).expect("sets");
    run(&setup.core, SET, &Value::Null).expect("sets");
    assert_eq!(setup.core.user_settings().ui.theme, Theme::System);
}

#[tokio::test]
async fn set_refuses_anything_but_the_three_and_changes_nothing() {
    let setup = setup(config());
    run(&setup.core, SET, &json!({ "theme": "light" })).expect("sets");
    for args in [
        json!({ "theme": "purple" }),
        json!({ "theme": "Dark" }),
        json!({ "theme": 1 }),
    ] {
        let error = run(&setup.core, SET, &args).expect_err("refused");
        assert!(matches!(error, CommandError::InvalidArgs(_)), "{error:?}");
    }
    assert_eq!(setup.core.user_settings().ui.theme, Theme::Light);
    assert!(matches!(
        run(&setup.core, "ui.theme.other", &Value::Null),
        Err(CommandError::UnknownCommand(_))
    ));
}

#[test]
fn each_theme_has_its_own_notice() {
    let keys: Vec<&str> = [Theme::System, Theme::Light, Theme::Dark]
        .into_iter()
        .map(notice_key)
        .collect();
    assert_eq!(
        keys,
        ["theme.name.system", "theme.name.light", "theme.name.dark"]
    );
    let CoreToShell::Notification(notification) = notice(Theme::Dark) else {
        panic!("a notification");
    };
    assert_eq!(notification.level, NotificationLevel::Info);
    assert_eq!(notification.message_key, "theme.name.dark");
}
