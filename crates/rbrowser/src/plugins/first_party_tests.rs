use std::collections::BTreeSet;
use std::path::Path;

use super::{CATEGORIES, category, ids, recommended};

/// Every first-party plugin folder but a test-only one is in `plugins/first-party.json`, and
/// nothing else is: the wizard groups and recommends exactly what a release publishes.
#[test]
fn the_list_names_every_first_party_plugin_folder() {
    let plugins = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let folders: BTreeSet<String> = std::fs::read_dir(&plugins)
        .expect("plugins folder")
        .flatten()
        .map(|entry| entry.path())
        .filter(|dir| dir.join("manifest.toml").is_file() && !dir.join("TEST-ONLY").exists())
        .filter_map(|dir| {
            dir.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .collect();
    assert_eq!(ids(), folders);
}

#[test]
fn every_plugin_has_a_known_category_and_some_are_recommended() {
    for id in ids() {
        let group = category(&id).expect("a category");
        assert!(CATEGORIES.contains(&group), "{id}: {group}");
    }
    let recommended = recommended();
    assert!(!recommended.is_empty());
    assert!(recommended.is_subset(&ids()));
    // What the core itself relies on is recommended: `rbrowser init`'s analyser, the recorder's
    // export, the sitemap capture.
    for id in ["breakpoint-analyzer", "recorder-export", "sitemap-capture"] {
        assert!(recommended.contains(id), "{id}");
    }
    assert_eq!(category("not-a-plugin"), None);
}
