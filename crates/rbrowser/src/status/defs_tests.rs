use rb_commands::lint::lint_defaults;
use rb_commands::{CommandDef, Platform, builtin, builtin_reserved};

use super::*;

/// The core commands the app registers besides this module's, the Panels group's included.
fn other_core_defs() -> Vec<CommandDef> {
    let mut defs = builtin::definitions();
    defs.extend(crate::commands::definitions().expect("the grid's definitions"));
    defs.extend(crate::sessions::defs::definitions().expect("sessions"));
    defs.extend(crate::breakpoint_keys::definitions().expect("breakpoint keys"));
    defs.extend(crate::bookmarks::definitions().expect("bookmarks"));
    defs.extend(crate::flows::definitions().expect("flows"));
    defs.extend(crate::vitals::defs::definitions().expect("vitals"));
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
fn both_commands_sit_in_the_panels_group_without_a_tool_of_their_own() {
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
                TOGGLE_BAR.to_owned(),
                vec!["Ctrl+Shift+Space P S".to_owned()],
                McpCoverage::Omitted(TOGGLE_OMITTED.to_owned())
            ),
            (
                HISTORY.to_owned(),
                vec!["Ctrl+Shift+Space P L".to_owned()],
                McpCoverage::Omitted(HISTORY_OMITTED.to_owned())
            ),
        ]
    );
    assert!(defs.iter().all(|def| def.group == Group::Panels));
    assert!(defs.iter().all(|def| is_own(def.id.as_str())));
    assert!(!is_own("status.other"));
}
