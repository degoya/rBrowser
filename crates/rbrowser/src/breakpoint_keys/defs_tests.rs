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
fn breakpoint_shortcuts_defaults_pass_the_lint_against_the_reserved_list() {
    let mut defs = other_core_defs();
    defs.extend(definitions().expect("definitions"));
    let issues: Vec<String> = lint_defaults(&defs, builtin_reserved(), Platform::Windows)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(issues.is_empty(), "shortcut lint:\n{}", issues.join("\n"));
}

#[test]
fn breakpoint_shortcuts_have_the_reserved_defaults_and_the_mcp_tool() {
    let defs = definitions().expect("definitions");
    let bindings = |id: &str| -> Vec<String> {
        defs.iter()
            .find(|def| def.id.as_str() == id)
            .unwrap_or_else(|| panic!("{id} is defined"))
            .default_bindings
            .iter()
            .map(ToString::to_string)
            .collect()
    };
    assert_eq!(
        bindings("view.set_breakpoint.2"),
        ["Ctrl+Shift+2", "Ctrl+Shift+Space V 2"]
    );
    assert_eq!(bindings(NEXT), ["Ctrl+Shift+Space ]"]);
    assert_eq!(bindings(PREV), ["Ctrl+Shift+Space ["]);
    assert_eq!(bindings(WIDER), ["Ctrl+Shift+Space ."]);
    assert_eq!(bindings(NARROWER), ["Ctrl+Shift+Space -"]);
    assert_eq!(bindings(RESET), ["Ctrl+Shift+Space 0"]);
    assert_eq!(bindings("breakpoints.detected.1"), ["Ctrl+Shift+Space B 1"]);
    assert_eq!(bindings(SET), ["Ctrl+Shift+Space V S"]);
    assert_eq!(defs.len(), 6 + 9 + 9);
    assert!(
        defs.iter()
            .all(|def| def.mcp == McpCoverage::Covered(MCP_TOOL.to_owned()))
    );
}

#[test]
fn breakpoint_shortcuts_know_their_own_ids() {
    assert!(is_own("view.set_breakpoint.9"));
    assert!(is_own("breakpoints.detected.1"));
    assert!(is_own(RESET));
    assert!(!is_own("view.set_breakpoint.0"));
    assert!(!is_own("view.set_breakpoint.10"));
    assert!(!is_own("breakpoints.analyze"));
    assert_eq!(numbered("view.set_breakpoint.3", SET_PREFIX), Some(3));
}
