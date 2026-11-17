use rb_commands::lint::lint_defaults;
use rb_commands::{Platform, builtin, builtin_reserved};

use super::*;

/// Every core command the app registers besides this module's.
fn other_core_defs() -> Vec<CommandDef> {
    let mut defs = builtin::definitions();
    defs.extend(crate::commands::definitions().expect("the grid's definitions"));
    defs.push(rb_diag::command_def().expect("diagnostics.create"));
    defs.extend(rb_plugin_host::core_command_defs().expect("plugins.*"));
    defs
}

#[test]
fn session_defaults_pass_the_lint_against_the_reserved_list() {
    let mut defs = other_core_defs();
    defs.extend(definitions().expect("definitions"));
    let issues: Vec<String> = lint_defaults(&defs, builtin_reserved(), Platform::Windows)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(issues.is_empty(), "shortcut lint:\n{}", issues.join("\n"));
}

#[test]
fn session_commands_have_their_chords_and_tools() {
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
                ASSIGN.to_owned(),
                vec!["Ctrl+Shift+Space E S".to_owned()],
                McpCoverage::Covered("session_assign".to_owned())
            ),
            (
                CLEAR.to_owned(),
                vec!["Ctrl+Shift+Space E K".to_owned()],
                McpCoverage::Covered("session_clear".to_owned())
            ),
        ]
    );
    assert!(defs.iter().all(|def| def.group == Group::Environments));
    assert!(
        defs.iter()
            .all(|def| def.title_key == format!("commands.{}", def.id.as_str()))
    );
}

#[test]
fn session_commands_know_their_own_ids() {
    assert!(is_own(ASSIGN));
    assert!(is_own(CLEAR));
    assert!(!is_own("environments.select"));
}
