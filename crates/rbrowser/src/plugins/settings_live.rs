//! The plugin keys that apply at once (RB-094-23): `plugins.crashLimit` goes to the health
//! counters the moment it changes, and `plugins.disabled`, `plugins.trustedKeys` and
//! `plugins.developmentMode` - which decide what loads - load the plugins again, as an install
//! does, instead of waiting for the next start. A load the first-run wizard holds back says the
//! change takes effect at the next start. `plugins.compileCache` (RB-094-30) applies to the next
//! compile - an install, an update, the next load - without a load of its own.

use rb_config::PluginSettings;

use crate::control::Core;

/// Whether the change from `before` to `after` changes which plugins load and how they are
/// checked.
#[must_use]
pub fn load_changed(before: &PluginSettings, after: &PluginSettings) -> bool {
    before.disabled != after.disabled
        || before.trusted_keys != after.trusted_keys
        || before.development_mode != after.development_mode
}

/// Whether the change from `before` to `after` switches the compile cache.
#[must_use]
pub fn compile_cache_changed(before: &PluginSettings, after: &PluginSettings) -> bool {
    before.compile_cache != after.compile_cache
}

/// The user settings changed from `before`: the crash limit and the compile cache apply, and the
/// plugins load again when what decides the load changed (off the calling thread).
pub fn on_user_settings_changed(core: &Core, before: &PluginSettings) {
    let after = core.user_settings().plugins;
    crate::analyzer::plugins::healths().set_crash_limit(after.crash_limit);
    if compile_cache_changed(before, &after) {
        rb_plugin_host::set_compile_cache(
            crate::analyzer::plugins::compile_cache_dir(&after).as_deref(),
        );
    }
    if load_changed(before, &after) {
        super::live::reload_in_background(core, |core, live| {
            if !live {
                super::notify_restart_once(core);
            }
        });
    }
}

#[cfg(test)]
#[path = "settings_live_tests.rs"]
mod tests;
