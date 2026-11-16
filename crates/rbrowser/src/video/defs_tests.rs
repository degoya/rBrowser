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
