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
    defs.extend(crate::compare::defs::definitions().expect("env compare"));
    defs.extend(crate::capture::clean_defs::definitions().expect("capture cleaning"));
    defs.push(rb_diag::command_def().expect("diagnostics.create"));
    defs.extend(rb_plugin_host::core_command_defs().expect("plugins.*"));
    defs
}

#[test]
fn the_default_passes_the_lint_against_the_reserved_list_and_the_others() {
    let mut defs = other_core_defs();
    defs.extend(definitions().expect("definitions"));
    let issues: Vec<String> = lint_defaults(&defs, builtin_reserved(), Platform::Windows)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(issues.is_empty(), "shortcut lint:\n{}", issues.join("\n"));
}

#[test]
fn the_command_is_in_capture_with_its_chord_and_tool() {
    let defs = definitions().expect("definitions");
    assert_eq!(defs.len(), 1);
    let def = &defs[0];
    assert_eq!(def.id.as_str(), ENGINES_COMMAND);
    assert_eq!(def.title_key, "commands.compare.engines");
    assert_eq!(def.group, Group::Capture);
    let chords: Vec<String> = def
        .default_bindings
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(chords, ["Ctrl+Shift+Space C Q"]);
    assert_eq!(def.mcp, McpCoverage::Covered("compare_engines".to_owned()));
}
