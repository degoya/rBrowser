//! The plugin loader for the bundled first-party plugins (RB-010-13 wires it, the coordinator's
//! decision of wave 0.1-w1b): the packages beside the executable (`<exe dir>/plugins/*.rbplugin`)
//! are verified as `rb-plugin-host` rules them - the compiled-in release key, unsigned packages
//! only in development mode (`plugins.developmentMode`), `plugins.disabled` skipped - and loaded
//! with their locales handed to `rb_i18n` ([`I18nLocales`]) and their settings schemas checked
//! against the settings registry ([`SchemaSink`]). Charge 6 adds the other two sources: the
//! user-installed plugins in `plugins/` beside the user settings, when that folder exists (their
//! keys pinned in `plugins.trustedKeys`), and the project's `.rbrowser/plugins/` - loaded only
//! when RB-010-09's [`crate::trust::ProjectTrust`] trusts the project at the load; otherwise each
//! is refused with `plugins.error.untrusted_workspace` and its capabilities, for the trust
//! dialog. Every plugin reaches the app through [`crate::plugins::services::AppServices`].
//!
//! The first load on first use; a call runs the plugin's export on the core's runtime from a
//! blocking thread ([`block_on`]), never on the UI thread: a thread without the runtime gets an
//! error, not a runtime of its own (RB-080-28). RB-091-08: at the very first start, while the
//! first-run wizard's plugin choice is open, that load waits for the answer ([`hold()`]); no
//! caller before it keeps an empty load. RB-092-06: the plugins are a snapshot every caller takes
//! as an [`Arc`]; an install, update, removal or a project's trust loads them again
//! ([`reload`]) and puts the new snapshot in place, and the window follows
//! (`crate::plugins::live`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};

use rb_config::UserSettings;
use rb_plugin_host::{
    LoadContext, LoadReport, LocaleSink as _, PluginHealths, PluginOptions, PluginSource,
    PluginVerifier, Registry, Sources, load_all,
};
use serde_json::Value;
use tokio::runtime::Handle;

#[path = "plugins_call.rs"]
mod call;
#[path = "plugins_hold.rs"]
mod hold;
#[path = "plugins_sinks.rs"]
mod sinks;
#[path = "plugins_snapshot.rs"]
mod snapshot;

pub use call::PluginError;
pub(crate) use call::{NoRuntime, block_on};
pub use hold::{abandon, held, hold, release};
pub use sinks::{I18nLocales, SchemaSink};

/// The folder of the bundled plugins beside the executable.
pub const BUNDLED_DIR: &str = "plugins";

/// The plugins of this process in effect now, replaced whole by a live change (RB-092-06).
static PLUGINS: snapshot::Snapshots<LoadedPlugins> = snapshot::Snapshots::new();
/// One load at a time - the first and every [`reload`].
static LOADING: Mutex<()> = Mutex::new(());
/// No plugin: what [`bundled`] gives while the first choice holds the load, never kept as it.
static NOTHING: LazyLock<Arc<LoadedPlugins>> = LazyLock::new(|| Arc::new(empty()));
/// The project whose `.rbrowser/plugins/` the load reads: the instance's one project.
static PROJECT: OnceLock<Option<PathBuf>> = OnceLock::new();
/// Every plugin's own health for the life of the process (RB-090-59).
static HEALTHS: OnceLock<PluginHealths> = OnceLock::new();

/// The health of every plugin of this process, by id: one per plugin, kept across loads, so a
/// suspension holds for every version of that plugin and never reaches another one. The
/// person's Resume (`plugins.resume`) goes through it.
pub fn healths() -> &'static PluginHealths {
    HEALTHS.get_or_init(PluginHealths::default)
}

/// The folder of the user-installed plugins: `plugins` beside the user settings.
pub fn user_dir() -> Option<PathBuf> {
    rb_config::user_settings_path().map(|settings| settings.with_file_name(BUNDLED_DIR))
}

/// The project whose `.rbrowser/plugins/` this process's load reads, when it has one.
pub fn project_root() -> Option<PathBuf> {
    PROJECT.get().cloned().flatten()
}

/// What a load gave: the plugins, the refusals and the settings schemas taken.
pub struct LoadedPlugins {
    pub report: LoadReport,
    pub registry: Registry,
    pub schemas: Vec<(String, Value)>,
    /// How many settings keys each plugin's schema registered, by plugin id (RB-080-51).
    pub settings_counts: BTreeMap<String, usize>,
}

/// The folder of the bundled plugins: `plugins` beside the executable (in the macOS bundle in
/// `Contents/Resources`, RB-090-15).
pub fn bundled_dir() -> Option<PathBuf> {
    crate::bundle::resource_dir(&std::env::current_exe().ok()?).map(|dir| dir.join(BUNDLED_DIR))
}

/// Loads every package in `dir` as a bundled plugin, under the rules of `user`.
pub fn load(dir: &Path, user: &UserSettings) -> LoadedPlugins {
    let sources = Sources {
        bundled: Some(dir.to_path_buf()),
        ..Sources::default()
    };
    load_sources(&sources, user)
}

/// The verifier of `user`'s rules: the release key, and the keys a person confirmed
/// (`plugins.trustedKeys`, each held to its fingerprint) - also the catalogue's install
/// (`crate::plugins::catalog`, RB-091-01).
pub(crate) fn verifier(user: &UserSettings) -> PluginVerifier {
    let verifier = match PluginVerifier::with_roots(user.plugins.development_mode) {
        Ok(verifier) => verifier,
        Err(error) => {
            tracing::error!(%error, "the plugin release key does not decode; no plugin loads");
            PluginVerifier::new(user.plugins.development_mode)
        }
    };
    for pinned in &user.plugins.trusted_keys {
        let key = rb_plugin_host::decode_public_key(&pinned.public_key)
            .ok()
            .filter(|key| rb_plugin_host::key_fingerprint(key) == pinned.fingerprint);
        match key {
            Some(key) => {
                if let Err(error) = verifier.trust_key(pinned.key_id.clone(), key) {
                    tracing::warn!(%error, "a confirmed plugin key was not trusted");
                }
            }
            None => tracing::warn!(
                key_id = pinned.key_id.as_str(),
                "a confirmed plugin key does not match its fingerprint; not trusted"
            ),
        }
    }
    verifier
}

/// Loads every package of `sources` under the rules of `user`.
pub fn load_sources(sources: &Sources, user: &UserSettings) -> LoadedPlugins {
    let verifier = verifier(user);
    let schemas = SchemaSink::new();
    let disabled: BTreeSet<String> = user
        .plugins
        .disabled
        .iter()
        .map(|id| id.0.clone())
        .collect();
    // The same store as the config's trust gate (`trust::load_config`).
    let trust = crate::trust::ProjectTrust::new(rb_config::trust::trust_store_path());
    let healths = healths();
    healths.set_crash_limit(user.plugins.crash_limit);
    let context = LoadContext {
        verifier: &verifier,
        trust: &trust,
        locales: &I18nLocales,
        settings: &schemas,
        options: PluginOptions {
            // Every host service behind the imports (Charge 6); a plugin reaches one only with
            // the capability its manifest grants.
            services: crate::plugins::services::shared(),
            ..PluginOptions::default()
        },
        // Each plugin its own health (RB-090-59), never one for all.
        healths,
        disabled,
    };
    let mut registry = Registry::new();
    let report = match block_on(load_all(sources, &context, &mut registry)) {
        Ok(report) => report,
        Err(error) => {
            tracing::error!(%error, "the plugins were not loaded");
            LoadReport::default()
        }
    };
    for refused in &report.refused {
        tracing::warn!(
            source = ?refused.source,
            path = %refused.path.display(),
            code = refused.failure.code().unwrap_or_default(),
            "a plugin was not loaded"
        );
    }
    for entry in &report.loaded {
        tracing::info!(
            plugin = %entry.plugin.manifest().id,
            source = ?entry.source,
            "plugin loaded"
        );
    }
    // What Settings > Plugins lists (RB-080-51): one line that says where the start looked.
    tracing::info!(
        bundled_dir = ?sources.bundled,
        user_dir = ?sources.user,
        loaded = report.loaded.len(),
        disabled = report.disabled.len(),
        refused = report.refused.len(),
        "plugins: the load finished"
    );
    drop(context);
    let settings_counts = schemas.counts();
    let schemas = schemas.into_schemas();
    let services = crate::plugins::services::shared();
    services.set_schemas(schemas.clone());
    services.set_project_plugins(
        report
            .loaded
            .iter()
            .filter(|entry| entry.source == PluginSource::Project)
            .map(|entry| entry.plugin.manifest().id.clone())
            .collect(),
    );
    LoadedPlugins {
        report,
        registry,
        schemas,
        settings_counts,
    }
}

/// The folders of this process's load: the bundled plugins - only the ones the person chose
/// (RB-091-04, `crate::plugins::bundle`) - the user-installed ones when their folder exists, and
/// the project's.
fn process_sources() -> Sources {
    let bundled = bundled_dir();
    // The bundle's manifests, read once: for the choice and for the load (review 8).
    let declared = bundled
        .as_deref()
        .map(rb_plugin_host::bundle_manifests)
        .unwrap_or_default();
    let chosen = bundled
        .as_ref()
        .and_then(|_| crate::plugins::bundle::chosen_for_load(&declared));
    Sources {
        chosen,
        declared,
        bundled,
        user: user_dir().filter(|dir| dir.is_dir()),
        project_root: PROJECT.get().cloned().flatten(),
    }
}

/// No plugin at all.
fn empty() -> LoadedPlugins {
    LoadedPlugins {
        report: LoadReport::default(),
        registry: Registry::new(),
        schemas: Vec::new(),
        settings_counts: BTreeMap::new(),
    }
}

/// The plugins of this process in effect now - bundled, user-installed, the project's - loaded
/// on first use with the user settings in effect then. While the first choice holds the load
/// ([`held`]) no plugin, and nothing is kept: the load comes after the answer (RB-091-08). The
/// snapshot stays whole for its holder while a live change puts another in place (RB-092-06).
pub fn bundled() -> Arc<LoadedPlugins> {
    if let Some(loaded) = PLUGINS.get() {
        return loaded;
    }
    // One decision under the gate's lock: the load begins (and a later hold is refused), or the
    // gate holds it (review of RB-091-08).
    if !hold::begin_load() {
        return Arc::clone(&NOTHING);
    }
    let _loading = rb_util::lock(&LOADING);
    // Another caller's load may have ended while this one waited for the lock.
    if let Some(loaded) = PLUGINS.get() {
        return loaded;
    }
    let loaded = Arc::new(load_process());
    PLUGINS.put(Arc::clone(&loaded));
    loaded
}

/// The process's load: its folders under the user settings in effect now.
fn load_process() -> LoadedPlugins {
    let user = crate::control::settings::load_user().settings;
    let sources = process_sources();
    if sources.bundled.is_none() && sources.user.is_none() && sources.project_root.is_none() {
        return empty();
    }
    load_sources(&sources, &user)
}

/// Loads the plugins again from their folders, as a start would - the person's choice, a newer
/// copy, `plugins.disabled`, the project's trust and the keys all apply anew - and puts the new
/// snapshot in effect (RB-092-06): an install, update or removal and a project trusted after the
/// start take effect without a restart. The locales of a plugin no longer loaded leave the
/// catalogue. `false` when nothing loaded: the first choice holds the load (its answer loads the
/// plugins) or the app quits. A blocking thread with the core's runtime entered; the window
/// follows with `crate::plugins::live::follow`.
pub fn reload() -> bool {
    if !hold::begin_load() {
        return false;
    }
    let _loading = rb_util::lock(&LOADING);
    let new = Arc::new(load_process());
    let old = PLUGINS.put(Arc::clone(&new));
    let kept = loaded_ids(&new);
    for gone in old.as_deref().map(loaded_ids).unwrap_or_default() {
        if !kept.contains(&gone) {
            I18nLocales.unregister(&gone);
        }
    }
    tracing::info!(
        generation = PLUGINS.generation(),
        loaded = new.report.loaded.len(),
        "plugins: loaded again; the new set is in effect"
    );
    true
}

/// The ids of the plugins `loaded` loaded.
fn loaded_ids(loaded: &LoadedPlugins) -> BTreeSet<String> {
    loaded
        .report
        .loaded
        .iter()
        .map(|entry| entry.plugin.manifest().id.clone())
        .collect()
}

/// How many snapshots were put in effect so far: 0 before the first load, one more with each
/// [`reload`] - for a cache built from the plugins.
pub fn generation() -> u64 {
    PLUGINS.generation()
}

/// [`bundled`] once the first choice is answered: waits for the answer (a blocking thread only).
/// The window's plugin runtime starts with it, so commands, panels, page scripts and MCP tools
/// come as at a normal start, where the load ends after the window is up as well.
pub fn bundled_when_answered() -> Arc<LoadedPlugins> {
    hold::wait_released();
    bundled()
}

/// [`bundled`], `None` while the first choice holds the load: for a cache built from the
/// plugins, which must not be built empty.
pub fn unless_held() -> Option<Arc<LoadedPlugins>> {
    if PLUGINS.get().is_none() && held() {
        return None;
    }
    Some(bundled())
}

/// The plugins in effect once a load is done; `None` while the first runs or before it started.
pub fn loaded() -> Option<Arc<LoadedPlugins>> {
    PLUGINS.get()
}

/// Starts loading the plugins on a thread of its own (compiling takes a moment) that borrows
/// `runtime`, the core's, for the load; `project` is the instance's project, whose
/// `.rbrowser/plugins/` loads under workspace trust.
pub fn preload(runtime: Handle, project: Option<PathBuf>) {
    let _ = PROJECT.set(project);
    ensure_loading(runtime);
}

/// Whether the load's thread was started.
static STARTED: AtomicBool = AtomicBool::new(false);

/// `true` for the one caller that takes `flag`: the one that starts the load.
pub(crate) fn claim(flag: &AtomicBool) -> bool {
    !flag.swap(true, Ordering::AcqRel)
}

/// Starts the load on its thread unless it runs or is done: Settings > Plugins asks for the
/// list and must get one even when nothing loaded the plugins yet (RB-080-51). Never waits; the
/// thread waits for the first choice's answer when it holds the load (RB-091-08).
pub fn ensure_loading(runtime: Handle) {
    if PLUGINS.get().is_some() || !claim(&STARTED) {
        return;
    }
    if let Err(error) = std::thread::Builder::new()
        .name("rb-plugins".to_owned())
        .spawn(move || {
            let _runtime = runtime.enter();
            let _ = bundled_when_answered();
        })
    {
        STARTED.store(false, Ordering::Release);
        tracing::warn!(%error, "the plugins are loaded on first use");
    }
}

#[cfg(test)]
#[path = "plugins_tests.rs"]
mod tests;
