//! Test support of the control (RB-094-10): a leader chord free for a test to bind - held by no
//! command of the registry and bound by no bundled plugin's manifest - so a test does not break
//! when a plugin or a core command takes the chord it once found free.

use std::path::Path;

use rb_commands::validate::bindings_conflict;
use rb_commands::{Binding, CommandId};

use super::Core;
use crate::cdp::lock;

/// The default bindings of every bundled plugin (`plugins/*/manifest.toml`, each `shortcut`).
pub(crate) fn plugin_bindings() -> Vec<Binding> {
    let plugins = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let mut bindings = Vec::new();
    let mut manifests = 0;
    for entry in std::fs::read_dir(plugins).expect("plugins/") {
        let manifest = entry.expect("entry").path().join("manifest.toml");
        let Ok(text) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        manifests += 1;
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            if key.trim() != "shortcut" {
                continue;
            }
            let value = value.trim().trim_matches('"');
            let binding = value
                .parse::<Binding>()
                .unwrap_or_else(|error| panic!("{}: {value}: {error}", manifest.display()));
            bindings.push(binding);
        }
    }
    assert!(manifests > 10, "the plugins were read: {manifests}");
    bindings
}

/// A chord of the View group (`Ctrl+Shift+Space V <key>`) that `id` may take in `core`'s
/// registry and that no bundled plugin binds by default.
pub(crate) fn free_chord(core: &Core, id: &str) -> String {
    let taken = plugin_bindings();
    let id = CommandId::new(id).expect("a command id");
    let registry = lock(&core.registry);
    // The letters and digits first; the group is nearly full, so the punctuation keys too.
    ('A'..='Z')
        .chain('0'..='9')
        .chain(['/', ';', '[', ']', '=', '\'', '`', '\\'])
        .map(|key| format!("Ctrl+Shift+Space V {key}"))
        .filter(|text| text.parse::<Binding>().is_ok())
        .find(|text| {
            let binding: Binding = text.parse().expect("a chord");
            !taken.iter().any(|held| bindings_conflict(held, &binding))
                && registry.check_for(&id, &binding).is_ok()
        })
        .expect("a free chord in the View group")
}

#[cfg(test)]
#[path = "test_support_tests.rs"]
mod tests;
