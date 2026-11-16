use rb_commands::lint::lint_defaults;
use rb_commands::{Platform, builtin, builtin_reserved};

use super::*;

/// Every core command the app registers besides this module's.
fn other_core_defs() -> Vec<CommandDef> {
    let mut defs = builtin::definitions();
    defs.extend(crate::commands::definitions().expect("the grid's definitions"));
    defs.extend(crate::sessions::defs::definitions().expect("sessions"));
    defs.extend(crate::breakpoint_keys::definitions().expect("breakpoint keys"));
    defs.push(rb_diag::command_def().expect("diagnostics.create"));
    defs.extend(rb_plugin_host::core_command_defs().expect("plugins.*"));
    defs
}

#[test]
fn bookmark_defaults_pass_the_lint_against_the_reserved_list() {
    let mut defs = other_core_defs();
    defs.extend(definitions().expect("definitions"));
    let issues: Vec<String> = lint_defaults(&defs, builtin_reserved(), Platform::Windows)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(issues.is_empty(), "shortcut lint:\n{}", issues.join("\n"));
}

#[test]
fn bookmark_commands_have_their_chords_and_tools() {
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
                OPEN.to_owned(),
                vec!["Ctrl+Shift+Space P B".to_owned()],
                McpCoverage::Covered("navigate".to_owned())
            ),
            (
                ADD.to_owned(),
                vec!["Ctrl+Shift+Space P K".to_owned()],
                McpCoverage::Covered("config_set".to_owned())
            ),
            (
                MAKE_RELATIVE.to_owned(),
                vec!["Ctrl+Shift+Space P P".to_owned()],
                McpCoverage::Covered("bookmarks_make_relative".to_owned())
            ),
        ]
    );
    assert!(defs.iter().all(|def| def.group == Group::Panels));
    assert!(
        defs.iter()
            .all(|def| def.title_key == format!("commands.{}", def.id.as_str()))
    );
}

#[test]
fn bookmark_commands_know_their_own_ids() {
    assert!(is_own(OPEN));
    assert!(is_own(ADD));
    assert!(is_own(MAKE_RELATIVE));
    assert!(!is_own("view.navigate"));
}
