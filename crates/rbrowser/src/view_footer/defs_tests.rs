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
fn the_toggles_sit_in_the_overlays_group_without_a_tool_of_their_own() {
    let defs = definitions().expect("definitions");
    let rows: Vec<(String, String, Vec<String>, McpCoverage)> = defs
        .iter()
        .map(|def| {
            assert_eq!(def.group, Group::Overlays);
            (
                def.id.as_str().to_owned(),
                def.title_key.clone(),
                def.default_bindings
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                def.mcp.clone(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            (
                TOGGLE.to_owned(),
                "commands.ui.view_footer.toggle".to_owned(),
                vec!["Ctrl+Shift+Space O B".to_owned()],
                McpCoverage::Omitted(TOGGLE_OMITTED.to_owned()),
            ),
            // RB-090-71: the vitals bar, `W` for Web Vitals.
            (
                VITALS_TOGGLE.to_owned(),
                "commands.ui.vitals_bar.toggle".to_owned(),
                vec!["Ctrl+Shift+Space O W".to_owned()],
                McpCoverage::Omitted(VITALS_OMITTED.to_owned()),
            ),
            // RB-090-73: the Lighthouse bar, `G` for its gauges.
            (
                LIGHTHOUSE_TOGGLE.to_owned(),
                "commands.ui.lighthouse_bar.toggle".to_owned(),
                vec!["Ctrl+Shift+Space O Q".to_owned()],
                McpCoverage::Omitted(LIGHTHOUSE_OMITTED.to_owned()),
            ),
        ]
    );
}

#[test]
fn each_toggle_switches_its_own_setting() {
    assert_eq!(setting_of(TOGGLE), Some(VISIBLE_KEY));
    assert_eq!(setting_of(VITALS_TOGGLE), Some(VITALS_KEY));
    assert_eq!(VITALS_KEY, "ui.viewFooter.vitals");
    assert_eq!(setting_of(LIGHTHOUSE_TOGGLE), Some(LIGHTHOUSE_KEY));
    assert_eq!(LIGHTHOUSE_KEY, "ui.viewFooter.lighthouse");
    assert_eq!(setting_of("ui.other"), None);
}

/// The overlay plugins' chords under `O` are theirs: `B`, `W` and `G` are none of them.
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
        for chord in [
            "\"Ctrl+Shift+Space O B\"",
            "\"Ctrl+Shift+Space O W\"",
            "\"Ctrl+Shift+Space O Q\"",
        ] {
            assert!(
                !text.contains(chord),
                "{} binds the footer's chord {chord}",
                manifest.display()
            );
        }
    }
    assert!(manifests > 10, "the plugins were read: {manifests}");
}
