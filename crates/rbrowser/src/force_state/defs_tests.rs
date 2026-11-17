use rb_commands::lint::lint_defaults;
use rb_commands::{McpCoverage, Platform, builtin, builtin_reserved};

use super::*;

/// The core commands - the DevTools group's among them - and the others the app registers.
fn other_core_defs() -> Vec<CommandDef> {
    let mut defs = builtin::definitions();
    defs.extend(crate::commands::definitions().expect("the grid's definitions"));
    defs.extend(crate::vitals::defs::definitions().expect("vitals"));
    defs.extend(crate::page_zoom::definitions().expect("page zoom"));
    defs.extend(crate::emulation::extras::definitions().expect("emulation extras"));
    defs.push(rb_diag::command_def().expect("diagnostics.create"));
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
fn both_commands_sit_in_the_devtools_group_with_the_tool() {
    let summary: Vec<(String, String, Group, Vec<String>, McpCoverage)> = definitions()
        .expect("definitions")
        .iter()
        .map(|def| {
            (
                def.id.as_str().to_owned(),
                def.title_key.clone(),
                def.group,
                def.default_bindings
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                def.mcp.clone(),
            )
        })
        .collect();
    let row = |id: &str, key: &str| {
        (
            id.to_owned(),
            format!("commands.{id}"),
            Group::Devtools,
            vec![format!("Ctrl+Shift+Space D {key}")],
            McpCoverage::Covered(TOOL.to_owned()),
        )
    };
    assert_eq!(summary, [row(SET, "H"), row(RESET, "Q")]);
    assert_eq!(SET, "view.force_state.set");
    assert_eq!(RESET, "view.force_state.reset");
    assert_eq!(TOOL, "views_force_state");
}
