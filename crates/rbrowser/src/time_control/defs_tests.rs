use rb_commands::lint::lint_defaults;
use rb_commands::{McpCoverage, Platform, builtin, builtin_reserved};

use super::*;

/// The core commands - the sync and overlay groups' among them - and the others the app
/// registers under `S` and `O`.
fn other_core_defs() -> Vec<CommandDef> {
    let mut defs = builtin::definitions();
    defs.extend(crate::commands::definitions().expect("the grid's definitions"));
    defs.extend(crate::sync_agent::page_commands::definitions().expect("page API"));
    defs.extend(crate::portal::definitions().expect("portal"));
    defs.extend(crate::view_footer::defs::definitions().expect("view footer"));
    defs.extend(crate::emulation::extras::definitions().expect("emulation extras"));
    defs.extend(crate::force_state::definitions().expect("force state"));
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
fn the_clock_sits_with_the_sync_and_the_animations_with_the_overlays() {
    let summary: Vec<(String, String, Group, Vec<String>, McpCoverage)> = definitions()
        .expect("definitions")
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
    assert_eq!(
        summary,
        [
            (
                CLOCK.to_owned(),
                "commands.emulation.clock".to_owned(),
                Group::Sync,
                vec!["Ctrl+Shift+Space S T".to_owned()],
                McpCoverage::Covered(CLOCK_TOOL.to_owned()),
            ),
            (
                ANIMATIONS.to_owned(),
                "commands.overlays.animations".to_owned(),
                Group::Overlays,
                vec!["Ctrl+Shift+Space O J".to_owned()],
                McpCoverage::Covered(ANIMATIONS_TOOL.to_owned()),
            ),
        ]
    );
    assert_eq!(CLOCK, "emulation.clock");
    assert_eq!(ANIMATIONS, "overlays.animations");
    assert_eq!(CLOCK_TOOL, "clock_set");
    assert_eq!(ANIMATIONS_TOOL, "animations_set");
}

/// The bundled plugins' chords are theirs: `O A` is the target-size overlay's, so the
/// animations have `O J`; neither chord is any plugin's.
#[test]
fn the_chords_are_free_among_the_bundled_plugins() {
    let plugins = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let mut manifests = 0;
    for entry in std::fs::read_dir(plugins).expect("plugins/") {
        let manifest = entry.expect("entry").path().join("manifest.toml");
        let Ok(text) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        manifests += 1;
        for chord in ["\"Ctrl+Shift+Space S T\"", "\"Ctrl+Shift+Space O J\""] {
            assert!(
                !text.contains(chord),
                "{} binds {chord}",
                manifest.display()
            );
        }
    }
    assert!(manifests > 10, "the plugins were read: {manifests}");
}
