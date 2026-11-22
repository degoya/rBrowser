use rb_config::{PluginId, PluginSettings};

use super::*;

/// RB-094-23: switching a plugin off or on, a trusted key and the development mode change what
/// loads - the plugins load again at once instead of at the next start.
#[test]
fn what_decides_the_load_reloads_and_nothing_else_does() {
    let before = PluginSettings::default();
    let mut after = before.clone();
    assert!(!load_changed(&before, &after));
    after.disabled.push(PluginId("seo".to_owned()));
    assert!(load_changed(&before, &after), "disabled");
    let mut after = before.clone();
    after.development_mode = !before.development_mode;
    assert!(load_changed(&before, &after), "development mode");
    let mut after = before.clone();
    after.crash_limit = before.crash_limit + 1;
    after.show_bundled = !before.show_bundled;
    after.page_scripts_disabled.push(PluginId("seo".to_owned()));
    assert!(
        !load_changed(&before, &after),
        "the crash limit and the switches of a loaded plugin apply without a load"
    );
}

/// RB-094-30: `plugins.compileCache` changes no plugin that runs - it applies to the next
/// compile, without a load of its own.
#[test]
fn the_compile_cache_applies_without_a_load() {
    let before = PluginSettings::default();
    let mut after = before.clone();
    after.compile_cache = !before.compile_cache;
    assert!(!load_changed(&before, &after));
    assert!(compile_cache_changed(&before, &after));
    assert!(!compile_cache_changed(&before, &before.clone()));
}
