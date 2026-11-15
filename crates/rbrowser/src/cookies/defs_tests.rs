use rb_commands::lint::lint_defaults;
use rb_commands::{Platform, builtin, builtin_reserved};

use super::*;

/// Every core command the app registers, this one among them.
fn core_defs() -> Vec<CommandDef> {
    let mut defs = builtin::definitions();
    defs.extend(crate::commands::definitions().expect("the grid's definitions"));
    defs.extend(crate::sessions::defs::definitions().expect("sessions"));
    defs.extend(crate::bookmarks::defs::definitions().expect("bookmarks"));
    defs.push(rb_diag::command_def().expect("diagnostics.create"));
    defs.extend(rb_plugin_host::core_command_defs().expect("plugins.*"));
    defs.extend(definitions().expect("definitions"));
    defs
}

#[test]
fn the_default_chord_passes_the_lint_and_collides_with_nothing() {
    let issues: Vec<String> = lint_defaults(&core_defs(), builtin_reserved(), Platform::Windows)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(issues.is_empty(), "shortcut lint:\n{}", issues.join("\n"));
}

#[test]
fn copy_cookies_is_leader_e_j_covered_by_cookies_copy() {
    let defs = definitions().expect("definitions");
    assert_eq!(defs.len(), 1);
    let def = &defs[0];
    assert_eq!(def.id.as_str(), "cookies.copy");
    assert_eq!(def.title_key, "commands.cookies.copy");
    assert_eq!(def.group, Group::Environments);
    let chords: Vec<String> = def
        .default_bindings
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(chords, vec!["Ctrl+Shift+Space E J".to_owned()]);
    assert_eq!(def.mcp, McpCoverage::Covered("cookies_copy".to_owned()));
}
