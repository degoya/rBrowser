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
    defs.extend(crate::sitemap::defs::definitions().expect("sitemap"));
    defs.push(rb_diag::command_def().expect("diagnostics.create"));
    defs.extend(rb_plugin_host::core_command_defs().expect("plugins.*"));
    defs
}

#[test]
fn video_defaults_pass_the_lint_against_the_reserved_list_and_the_others() {
    let mut defs = other_core_defs();
    defs.extend(definitions().expect("definitions"));
    let issues: Vec<String> = lint_defaults(&defs, builtin_reserved(), Platform::Windows)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(issues.is_empty(), "shortcut lint:\n{}", issues.join("\n"));
}

#[test]
fn video_commands_have_their_chords_and_mcp_entries() {
    let summary: Vec<(String, Vec<String>, McpCoverage)> = definitions()
        .expect("definitions")
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
    let row = |id: &str, chord: &str, mcp: McpCoverage| {
        (
            id.to_owned(),
            vec![format!("Ctrl+Shift+Space {chord}")],
            mcp,
        )
    };
    let covered = || McpCoverage::Covered(TOOL_NAME.to_owned());
    assert_eq!(
        summary,
        [
            row(TOGGLE, "C V", covered()),
            row(TOGGLE_ALL, "C W", covered()),
            row(PAUSE, "C Z", covered()),
            row(
                INSTALL,
                "X F",
                McpCoverage::Omitted(INSTALL_OMITTED.to_owned())
            ),
            row(
                REMOVE,
                "X G",
                McpCoverage::Omitted(REMOVE_OMITTED.to_owned())
            ),
            row(PANEL, "C T", McpCoverage::Omitted(PANEL_OMITTED.to_owned())),
        ]
    );
}

/// The download is the person's: no command of this module names an MCP tool that installs.
#[test]
fn the_install_is_not_reachable_over_mcp() {
    let install = definitions()
        .expect("definitions")
        .into_iter()
        .find(|def| def.id.as_str() == INSTALL)
        .expect("install");
    assert!(matches!(install.mcp, McpCoverage::Omitted(_)));
}

/// The context menu records the clicked view, not the active one (RB-080-43).
#[test]
fn the_toggle_takes_the_view_of_its_arguments() {
    assert_eq!(target_of(&json!({})), Ok(None), "the active view");
    assert_eq!(target_of(&Value::Null), Ok(None));
    assert_eq!(
        target_of(&json!({ "target": 3 })),
        Ok(Some(Target::View(rb_engine::ViewId(3))))
    );
    assert_eq!(
        target_of(&json!({ "target": "all" })),
        Ok(Some(Target::All))
    );
    assert!(matches!(
        target_of(&json!({ "target": "mobile" })),
        Err(CommandError::InvalidArgs(_))
    ));
}

/// RB-090-71 item 2: `capture.video.panel` from the menus opened nothing while the panels were
/// folded to their rail (`panels.dockState`, `rail` by default): the open kept the fold, as a
/// check's does. The person's command opens the dock at the Videos panel.
#[tokio::test]
async fn the_panel_command_opens_a_folded_dock_at_the_videos() {
    let mut user = rb_config::UserSettings::default();
    user.panels.dock = rb_config::PanelDock::Right;
    user.panels.dock_state = rb_config::PanelDockState::Rail;
    let setup = crate::panel_dock::tests::setup_with(user);
    assert_eq!(run(&setup.core, PANEL, &json!({})), Ok(Value::Null));
    let dock = crate::panel_dock::tests::docks(&setup.ui)
        .last()
        .cloned()
        .flatten()
        .expect("docked");
    assert_eq!(
        (dock.url.as_str(), dock.collapsed),
        ("rbrowser://ui/settings.html#/panels/videos", false)
    );
}

/// RB-090-68: `capture.video.pause` toggles without arguments; `paused` asks for a state.
#[test]
fn the_pause_takes_a_wanted_state_or_toggles() {
    use super::super::pause::PauseRequest;
    assert_eq!(pause_request_of(&json!({})), Ok(PauseRequest::Toggle));
    assert_eq!(pause_request_of(&Value::Null), Ok(PauseRequest::Toggle));
    assert_eq!(
        pause_request_of(&json!({ "paused": true })),
        Ok(PauseRequest::Pause)
    );
    assert_eq!(
        pause_request_of(&json!({ "paused": false })),
        Ok(PauseRequest::Resume)
    );
    assert!(matches!(
        pause_request_of(&json!({ "paused": "yes" })),
        Err(CommandError::InvalidArgs(_))
    ));
}

/// Without a recording the pause says so, in the command and in MCP.
#[tokio::test]
async fn the_pause_without_a_recording_is_refused() {
    let setup = crate::panel_dock::tests::setup_with(rb_config::UserSettings::default());
    assert!(matches!(
        run(&setup.core, PAUSE, &json!({})),
        Err(CommandError::Failed(_))
    ));
    let refused = host_tool(&setup.core, json!({ "action": "pause" }))
        .await
        .expect_err("no recording runs");
    assert_eq!(refused.code, "video.not_recording");
}
