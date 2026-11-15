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
fn env_compare_defaults_pass_the_lint_against_the_reserved_list_and_the_others() {
    let mut defs = other_core_defs();
    defs.extend(definitions().expect("definitions"));
    let issues: Vec<String> = lint_defaults(&defs, builtin_reserved(), Platform::Windows)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(issues.is_empty(), "shortcut lint:\n{}", issues.join("\n"));
}

#[test]
fn env_compare_commands_have_their_chords_and_tools() {
    let summary: Vec<(String, Vec<String>, McpCoverage, Group)> = definitions()
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
                def.group,
            )
        })
        .collect();
    let row = |id: &str, chord: &str, tool: &str| {
        (
            id.to_owned(),
            vec![format!("Ctrl+Shift+Space {chord}")],
            McpCoverage::Covered(tool.to_owned()),
            Group::Environments,
        )
    };
    assert_eq!(
        summary,
        [
            row(LIVE, "E C", "env_compare_live"),
            row(RUN, "E R", "env_compare"),
            row(REPORT, "E O", "env_compare_result"),
            row(CANCEL, "E X", "env_compare_cancel"),
        ]
    );
}
