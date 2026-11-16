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
    defs.extend(crate::video::defs::definitions().expect("video"));
    defs.push(rb_diag::command_def().expect("diagnostics.create"));
    defs.extend(rb_plugin_host::core_command_defs().expect("plugins.*"));
    defs
}

#[test]
fn extension_defaults_pass_the_lint_against_the_reserved_list_and_the_others() {
    let mut defs = other_core_defs();
    defs.extend(definitions().expect("definitions"));
    let issues: Vec<String> = lint_defaults(&defs, builtin_reserved(), Platform::Windows)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(issues.is_empty(), "shortcut lint:\n{}", issues.join("\n"));
}

/// No command reaches an install over MCP: adding a folder or a project's recommendation is
/// the person's own command, and only `extension_list` reads the state. The removal
/// (RB-080-30) is covered by `extension_remove`, which asks the person in the window.
#[test]
fn extension_commands_have_their_chords_and_no_mcp_install() {
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
    let row = |id: &str, chord: &str, reason: &str| {
        (
            id.to_owned(),
            vec![format!("Ctrl+Shift+Space {chord}")],
            McpCoverage::Omitted(reason.to_owned()),
        )
    };
    assert_eq!(
        summary,
        [
            row(MANAGE, "X M", MANAGE_OMITTED),
            row(LOAD_UNPACKED, "X O", INSTALL_OMITTED),
            row(ADD_RECOMMENDED, "X A", INSTALL_OMITTED),
            row(INSTALL_WEB_STORE, "X W", WEB_STORE_OMITTED),
            row(CHECK_UPDATES, "X C", UPDATES_OMITTED),
            (
                REMOVE.to_owned(),
                vec!["Ctrl+Shift+Space X X".to_owned()],
                McpCoverage::Covered(web::REMOVE_TOOL.to_owned()),
            ),
        ]
    );
    for def in definitions().expect("definitions") {
        assert_eq!(def.group, Group::Plugins, "{}", def.id.as_str());
        assert_eq!(def.title_key, format!("commands.{}", def.id.as_str()));
    }
}

#[test]
fn the_list_tool_takes_no_arguments() {
    assert!(tool_args(&Value::Null).is_ok());
    assert!(tool_args(&json!({})).is_ok());
    assert!(tool_args(&json!({ "path": "C:\\x" })).is_err());
    assert!(tool_args(&json!([1])).is_err());
}
