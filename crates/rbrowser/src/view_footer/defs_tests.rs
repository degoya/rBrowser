use rb_commands::lint::lint_defaults;
use rb_commands::{CommandDef, Platform, builtin, builtin_reserved};

use super::*;

/// The core commands the app registers besides this module's.
fn other_core_defs() -> Vec<CommandDef> {
    let mut defs = builtin::definitions();
    defs.extend(crate::commands::definitions().expect("the grid's definitions"));
    defs.extend(crate::sessions::defs::definitions().expect("sessions"));
    defs.extend(crate::breakpoint_keys::definitions().expect("breakpoint keys"));
    defs.extend(crate::bookmarks::definitions().expect("bookmarks"));
    defs.extend(crate::flows::definitions().expect("flows"));
    defs.extend(crate::vitals::defs::definitions().expect("vitals"));
    defs.extend(crate::status::defs::definitions().expect("status"));
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
fn the_toggle_sits_in_the_overlays_group_without_a_tool_of_its_own() {
    let defs = definitions().expect("definitions");
    let [def] = defs.as_slice() else {
        panic!("one command, got {}", defs.len());
    };
    assert_eq!(def.id.as_str(), TOGGLE);
    assert_eq!(def.title_key, "commands.ui.view_footer.toggle");
    assert_eq!(def.group, Group::Overlays);
    assert_eq!(
        def.default_bindings
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["Ctrl+Shift+Space O B"]
    );
    assert_eq!(def.mcp, McpCoverage::Omitted(TOGGLE_OMITTED.to_owned()));
}

/// The overlay plugins' chords under `O` are theirs: `B` is none of them.
#[test]
fn the_chord_is_free_among_the_bundled_plugins() {
    let plugins = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let mut manifests = 0;
    for entry in std::fs::read_dir(plugins).expect("plugins/") {
        let manifest = entry.expect("entry").path().join("manifest.toml");
        let Ok(text) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        manifests += 1;
        assert!(
            !text.contains("\"Ctrl+Shift+Space O B\""),
            "{} binds the footer's chord",
            manifest.display()
        );
    }
    assert!(manifests > 10, "the plugins were read: {manifests}");
}
