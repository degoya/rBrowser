use rb_commands::lint::lint_defaults;
use rb_commands::{Platform, builtin, builtin_reserved};

use super::*;

/// Every core command the app registers besides this module's, the Reload group's included.
fn other_core_defs() -> Vec<CommandDef> {
    let mut defs = builtin::definitions();
    defs.extend(crate::commands::definitions().expect("the grid's definitions"));
    defs.extend(crate::sessions::defs::definitions().expect("sessions"));
    defs.extend(crate::bookmarks::definitions().expect("bookmarks"));
    defs.extend(crate::sidecar::definitions().expect("sidecar"));
    defs.extend(crate::breakpoint_keys::definitions().expect("breakpoint keys"));
    defs.push(rb_diag::command_def().expect("diagnostics.create"));
    defs.extend(rb_plugin_host::core_command_defs().expect("plugins.*"));
    defs
}

#[test]
fn the_chord_passes_the_lint_against_the_reserved_list_and_the_other_commands() {
    let mut defs = other_core_defs();
    defs.extend(definitions().expect("definitions"));
    let issues: Vec<String> = lint_defaults(&defs, builtin_reserved(), Platform::Windows)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(issues.is_empty(), "shortcut lint:\n{}", issues.join("\n"));
}

#[test]
fn the_command_has_its_chord_group_title_and_tool() {
    let defs = definitions().expect("definitions");
    let [def] = defs.as_slice() else {
        panic!("one command: {defs:?}");
    };
    assert_eq!(def.id.as_str(), "views.restore_last");
    assert_eq!(def.title_key, "commands.views.restore_last");
    assert_eq!(def.group, Group::Reload);
    let bindings: Vec<String> = def
        .default_bindings
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(bindings, ["Ctrl+Shift+Space R L"]);
    assert_eq!(
        def.mcp,
        McpCoverage::Covered("views_restore_last".to_owned())
    );
}
