//! Plugins installed, updated, removed or trusted while the app runs (RB-092-06): the change
//! takes effect at once, without a restart.
//!
//! An install, update or removal (the catalogue, the first-run wizard, the update check) and a
//! project trusted after the start call [`reload`]: `crate::analyzer::plugins::reload` loads the
//! plugins again from their folders, as a start would, and puts the new snapshot in effect; then
//! the window follows ([`follow`]). What changed is told apart by plugin id ([`diff`]): a plugin
//! added, replaced (another package, version, manifest or files) or removed. The window's
//! runtime serves the new set; page scripts that did not change run on, a replaced plugin's are
//! stopped and injected anew, a removed one's stopped; the lasting CDP state of a removed or
//! replaced plugin is reset on the views; the commands and shortcuts of the removed
//! and replaced plugins leave the registry and those of the added and replaced come in; the
//! panels, the settings sources and the Settings > Plugins lists are told; MCP lists the new
//! tools (`notifications/tools/list_changed` through the tools' revision); checks, the context
//! menu and the settings read the snapshot at each use. A call under way keeps the plugins it
//! took and ends without a crash; a plugin removed meanwhile answers `plugins.error.not_available`.
//! One window follows one change at a time, never while its runtime starts.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use rb_ipc::CoreToShell;
use rb_plugin_host::{LoadedEntry, PluginManifest, PluginSource};
use serde_json::{Value, json};

use crate::cdp::lock;
use crate::control::Core;

/// Held while a window's runtime starts or follows a change.
pub(super) static FOLLOWING: Mutex<()> = Mutex::new(());

/// The `pluginPanel` event that tells the shell pages the installed plugins changed: Settings >
/// Plugins and the catalogue ask for their lists again.
#[must_use]
pub fn changed_message() -> Value {
    json!({ "pluginsChanged": true })
}

/// What a live change did, by plugin id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Change {
    pub added: BTreeSet<String>,
    pub replaced: BTreeSet<String>,
    pub removed: BTreeSet<String>,
}

impl Change {
    /// Nothing changed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.replaced.is_empty() && self.removed.is_empty()
    }

    /// The plugins removed or replaced: their commands leave.
    #[must_use]
    pub fn gone(&self) -> BTreeSet<String> {
        self.removed.union(&self.replaced).cloned().collect()
    }

    /// The plugins whose lasting CDP state on the views (a vision deficiency, scripts disabled,
    /// an overlay) is reset: the removed and the replaced ones - an update starts clean.
    #[must_use]
    pub fn cdp_reset(&self) -> BTreeSet<String> {
        self.gone()
    }

    /// The plugins added or replaced: their commands come in.
    #[must_use]
    pub fn came(&self) -> BTreeSet<String> {
        self.added.union(&self.replaced).cloned().collect()
    }
}

/// The change from `before` to `now`, each a list of plugins by id with what tells two of them
/// apart ([`shapes`] in the app).
#[must_use]
pub fn diff<K: PartialEq>(before: &[(String, K)], now: &[(String, K)]) -> Change {
    let mut change = Change::default();
    for (id, shape) in now {
        match before.iter().find(|(old, _)| old == id) {
            None => {
                change.added.insert(id.clone());
            }
            Some((_, old)) if old != shape => {
                change.replaced.insert(id.clone());
            }
            Some(_) => {}
        }
    }
    for (id, _) in before {
        if !now.iter().any(|(new, _)| new == id) {
            change.removed.insert(id.clone());
        }
    }
    change
}

/// What a loaded plugin is for [`diff`]: where its package lies, what its manifest declares and
/// the page and panel files it carries.
#[derive(PartialEq)]
pub struct Shape<'a> {
    source: PluginSource,
    path: &'a Path,
    manifest: &'a PluginManifest,
    ui_files: &'a BTreeMap<String, Vec<u8>>,
}

/// The plugins of `loaded` by id, for [`diff`].
#[must_use]
pub fn shapes(loaded: &[LoadedEntry]) -> Vec<(String, Shape<'_>)> {
    loaded
        .iter()
        .map(|entry| {
            let manifest = entry.plugin.manifest();
            let shape = Shape {
                source: entry.source,
                path: &entry.path,
                manifest,
                ui_files: entry.ui_files.as_ref(),
            };
            (manifest.id.clone(), shape)
        })
        .collect()
}

/// Loads the plugins again and brings the window to them: an install, update or removal takes
/// effect now. `false` when nothing loaded - the first-run wizard's plugin choice holds the load
/// (its answer loads the plugins) or the app quits; the caller then says the change takes effect
/// at the next start. Blocking: a blocking thread of the core's runtime.
pub fn reload(core: &Core) -> bool {
    if !crate::analyzer::plugins::reload() {
        return false;
    }
    if let Some(core) = core.arc() {
        follow(&core);
    }
    true
}

/// [`reload`] on a blocking thread of the core's runtime, never on the calling one (a trust or
/// config change comes on the UI thread); `done` gets whether the plugins loaded live.
pub fn reload_in_background(core: &Core, done: impl FnOnce(&Arc<Core>, bool) + Send + 'static) {
    let Some(core) = core.arc() else {
        return;
    };
    let runtime = core.runtime().clone();
    drop(runtime.spawn_blocking(move || {
        let live = reload(&core);
        done(&core, live);
    }));
}

/// A project trusted after the start: its plugins load now, once per set of plugins in effect -
/// a load that refuses them again (the trust store disagrees) is not repeated until the plugins
/// change. Without a live load the notice says they load at the next start.
pub fn reload_for_trust(core: &Core) {
    static TRIED: AtomicU64 = AtomicU64::new(u64::MAX);
    let generation = crate::analyzer::plugins::generation();
    if TRIED.swap(generation, Ordering::AcqRel) == generation {
        return;
    }
    reload_in_background(core, |core, live| {
        TRIED.store(crate::analyzer::plugins::generation(), Ordering::Release);
        if !live {
            super::notify_restart_once(core);
        }
    });
}

/// Brings `core`'s window to the plugins in effect now, from the ones it followed so far. Before
/// its runtime started nothing: the start takes the plugins in effect then.
pub fn follow(core: &Arc<Core>) {
    let _following = lock(&FOLLOWING);
    let Some(now) = crate::analyzer::plugins::loaded() else {
        return;
    };
    let Some((runtime, before)) = super::take_followed(core, &now) else {
        return;
    };
    if Arc::ptr_eq(&before, &now) {
        return;
    }
    let change = diff(&shapes(&before.report.loaded), &shapes(&now.report.loaded));
    tracing::info!(
        added = change.added.len(),
        replaced = change.replaced.len(),
        removed = change.removed.len(),
        "plugins: the window follows the new set"
    );
    runtime.replace_plugins(super::runtime::plugins_of(&now.report.loaded));
    // A removed or replaced plugin leaves no lasting emulation on the views.
    super::services::shared().reset_cdp(change.cdp_reset().iter().map(String::as_str));
    // The page scripts follow the new set, the switches and the trust.
    let panels = super::apply_switches(core, &runtime);
    super::commands::follow(
        core,
        &runtime,
        &now.report.loaded,
        &change.gone(),
        &change.came(),
    );
    if runtime.plugins().iter().any(|plugin| plugin.console) {
        super::console::start(core);
    }
    // A consent check installed live probes as one loaded at the start (RB-094-09).
    super::consent::follow(core, &runtime.plugins());
    crate::api_client::follow(core, &runtime.plugins());
    crate::load_test::follow(core, &runtime.plugins());
    super::follow_network(core, runtime);
    super::announce_panels(core, panels);
    core.send(&CoreToShell::SettingSourcesChanged);
    core.send(&CoreToShell::PluginPanel {
        message: changed_message(),
    });
}

#[cfg(test)]
#[path = "live_tests.rs"]
mod tests;
