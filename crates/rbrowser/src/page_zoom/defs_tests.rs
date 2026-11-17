use rb_commands::lint::lint_defaults;
use rb_commands::{McpCoverage, Platform, builtin, builtin_reserved};

use super::*;

/// The core commands of the View group and the others the app registers.
fn other_core_defs() -> Vec<CommandDef> {
    let mut defs = builtin::definitions();
    defs.extend(crate::commands::definitions().expect("the grid's definitions"));
    defs.extend(crate::sessions::defs::definitions().expect("sessions"));
    defs.extend(crate::breakpoint_keys::definitions().expect("breakpoint keys"));
    defs.extend(crate::emulation::extras::definitions().expect("emulation extras"));
    defs.extend(crate::app_theme::defs::definitions().expect("app theme"));
    defs.extend(crate::bookmarks::definitions().expect("bookmarks"));
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
fn the_three_commands_sit_in_the_view_group_with_the_tool() {
    let defs = definitions().expect("definitions");
    let summary: Vec<(String, String, Group, Vec<String>, McpCoverage)> = defs
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
            Group::View,
            vec![format!("Ctrl+Shift+Space V {key}")],
            McpCoverage::Covered(TOOL.to_owned()),
        )
    };
    assert_eq!(
        summary,
        [
            row(RESET, "Home"),
            row(ZOOM_IN, "Up"),
            row(ZOOM_OUT, "Down")
        ]
    );
    assert_eq!(RESET, "view.page_zoom.reset");
    assert_eq!(ZOOM_IN, "view.page_zoom.in");
    assert_eq!(ZOOM_OUT, "view.page_zoom.out");
    assert_eq!(TOOL, "view_page_zoom");
}

#[test]
fn the_page_zoom_commands_are_not_the_grids_zoom() {
    // The grid's zoom of the cell (`view.zoom_in`) and the page's zoom are two commands.
    let grid: Vec<String> = crate::commands::definitions()
        .expect("the grid's definitions")
        .iter()
        .map(|def| def.id.as_str().to_owned())
        .collect();
    for def in definitions().expect("definitions") {
        assert!(!grid.contains(&def.id.as_str().to_owned()));
    }
}
