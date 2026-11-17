use rb_commands::lint::lint_defaults;
use rb_commands::{Platform, builtin, builtin_reserved};

use super::*;

/// Every core command the app registers besides this module's.
fn other_core_defs() -> Vec<CommandDef> {
    let mut defs = builtin::definitions();
    defs.extend(crate::commands::definitions().expect("the grid's definitions"));
    defs.extend(crate::sessions::defs::definitions().expect("sessions"));
    defs.extend(crate::bookmarks::defs::definitions().expect("bookmarks"));
    defs.extend(crate::breakpoint_keys::definitions().expect("breakpoint keys"));
    defs.push(rb_diag::command_def().expect("diagnostics.create"));
    defs.extend(rb_plugin_host::core_command_defs().expect("plugins.*"));
    defs
}

#[test]
fn layout_mode_defaults_pass_the_lint_against_the_reserved_list() {
    let mut defs = other_core_defs();
    defs.extend(definitions().expect("definitions"));
    let issues: Vec<String> = lint_defaults(&defs, builtin_reserved(), Platform::Windows)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(issues.is_empty(), "shortcut lint:\n{}", issues.join("\n"));
}

#[test]
fn the_job_files_chords_and_tools() {
    let defs = definitions().expect("definitions");
    let summary: Vec<(String, String, McpCoverage)> = defs
        .iter()
        .map(|def| {
            (
                def.id.as_str().to_owned(),
                def.default_bindings
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
                def.mcp.clone(),
            )
        })
        .collect();
    let covered = |tool: &str| McpCoverage::Covered(tool.to_owned());
    let chord = |keys: &str| format!("Ctrl+Shift+Space {keys}");
    assert_eq!(
        summary,
        [
            (MODE_GRID.into(), chord("L G"), covered("layout_set")),
            (MODE_HORIZONTAL.into(), chord("L H"), covered("layout_set")),
            (MODE_SPLIT.into(), chord("L S"), covered("layout_set")),
            (MODE_CANVAS.into(), chord("L C"), covered("layout_set")),
            (
                MINIMAP_TOGGLE.into(),
                chord("L M"),
                McpCoverage::Omitted(MINIMAP_OMITTED.into())
            ),
            (SPLIT_SWAP.into(), chord("L W"), covered("layout_set")),
            (SPLIT_GROW.into(), chord("L Right"), covered("layout_set")),
            (SPLIT_SHRINK.into(), chord("L Left"), covered("layout_set")),
            (
                CENTER.into(),
                chord("L ."),
                McpCoverage::Omitted(CENTER_OMITTED.into())
            ),
            (CANVAS_PLACE.into(), chord("L P"), covered("canvas_place")),
            (
                SAVE_PRESET.into(),
                chord("L N"),
                covered("layout_save_preset")
            ),
            (
                CANVAS_ZOOM_RESET.into(),
                chord("L 0"),
                covered("canvas_zoom")
            ),
            (
                CANVAS_ZOOM_IN.into(),
                chord("L Plus"),
                McpCoverage::Omitted(ZOOM_STEP_OMITTED.into())
            ),
            (
                CANVAS_ZOOM_OUT.into(),
                chord("L -"),
                McpCoverage::Omitted(ZOOM_STEP_OMITTED.into())
            ),
        ]
    );
    assert!(defs.iter().all(|def| def.group == Group::Layout));
}

#[test]
fn the_module_knows_its_own_ids() {
    assert!(is_own(MODE_CANVAS));
    assert!(is_own(CANVAS_PLACE));
    assert!(is_own(SAVE_PRESET));
    assert!(is_own(CANVAS_ZOOM_RESET));
    assert!(!is_own("layout.focus_mode"));
}
