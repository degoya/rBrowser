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
//! One load per process, on first use; a call runs the plugin's export on the core's runtime
//! from a blocking thread ([`block_on`]), never on the UI thread: a thread without the runtime
//! gets an error, not a runtime of its own (RB-080-28).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use rb_config::UserSettings;
use rb_plugin_host::{
    HostServices as _, LoadContext, LoadReport, LocaleSink, PluginHealth, PluginOptions,
    PluginSource, PluginVerifier, Registry, SettingsSink, Sources, load_all,
};
use rb_settings::SettingsRegistry;
use serde_json::Value;
use tokio::runtime::Handle;

use crate::cdp::lock;

/// The folder of the bundled plugins beside the executable.
pub const BUNDLED_DIR: &str = "plugins";

/// The plugins of this process.
static BUNDLED: OnceLock<LoadedPlugins> = OnceLock::new();
/// The project whose `.rbrowser/plugins/` the load reads: the instance's one project.
static PROJECT: OnceLock<Option<PathBuf>> = OnceLock::new();

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

/// Locales go to the one catalogue of `rb_i18n`.
pub struct I18nLocales;

impl LocaleSink for I18nLocales {
    fn register(&self, plugin_id: &str, language: &str, bytes: &[u8]) -> Result<(), String> {
        rb_i18n::register_plugin_locale(plugin_id, language, bytes)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    fn unregister(&self, plugin_id: &str) {
        rb_i18n::unregister_plugin_locales(plugin_id);
    }
}

/// A plugin's settings schema, checked against a registry holding the core keys (an unknown
/// category, a key that is taken or one without its annotations is refused) and kept for the
/// settings window.
pub struct SchemaSink {
    registry: Mutex<SettingsRegistry>,
    taken: Mutex<Vec<(String, Value)>>,
    counts: Mutex<BTreeMap<String, usize>>,
}

impl SchemaSink {
    /// A sink over the core schemas.
    pub fn new() -> Self {
        Self {
            registry: Mutex::new(crate::control::settings::settings_registry().0),
            taken: Mutex::new(Vec::new()),
            counts: Mutex::new(BTreeMap::new()),
        }
    }

    /// How many keys each plugin registered, by plugin id.
    pub fn counts(&self) -> BTreeMap<String, usize> {
        lock(&self.counts).clone()
    }

    /// The schemas taken, by plugin id.
    pub fn into_schemas(self) -> Vec<(String, Value)> {
        self.taken
            .into_inner()
            .unwrap_or_else(|poison| poison.into_inner())
    }
}

impl Default for SchemaSink {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsSink for SchemaSink {
    fn register_plugin_schema(&self, plugin_id: &str, schema: &Value) -> Result<usize, String> {
        let count = lock(&self.registry)
            .register_plugin_schema(plugin_id, schema)
            .map_err(|issues| {
                issues
                    .iter()
                    .map(|issue| format!("{}: {}", issue.key, issue.error))
                    .collect::<Vec<_>>()
                    .join("; ")
            })?;
        lock(&self.taken).push((plugin_id.to_owned(), schema.clone()));
        lock(&self.counts).insert(plugin_id.to_owned(), count);
        Ok(count)
    }
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
/// (`plugins.trustedKeys`, each held to its fingerprint).
fn verifier(user: &UserSettings) -> PluginVerifier {
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
    let context = LoadContext {
        verifier: &verifier,
        trust: &trust,
        locales: &I18nLocales,
        settings: &schemas,
        options: PluginOptions {
            health: std::sync::Arc::new(PluginHealth::new(user.plugins.crash_limit)),
            // Every host service behind the imports (Charge 6); a plugin reaches one only with
            // the capability its manifest grants.
            services: crate::plugins::services::shared(),
            ..PluginOptions::default()
        },
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

/// The folders of this process's load: the bundled plugins, the user-installed ones when their
/// folder exists, and the project's.
fn process_sources() -> Sources {
    Sources {
        bundled: bundled_dir(),
        user: user_dir().filter(|dir| dir.is_dir()),
        project_root: PROJECT.get().cloned().flatten(),
    }
}

/// The plugins of this process - bundled, user-installed, the project's - loaded on first use
/// with the user settings in effect then.
pub fn bundled() -> &'static LoadedPlugins {
    BUNDLED.get_or_init(|| {
        let user = crate::control::settings::load_user().settings;
        let sources = process_sources();
        if sources.bundled.is_none() && sources.user.is_none() && sources.project_root.is_none() {
            return LoadedPlugins {
                report: LoadReport::default(),
                registry: Registry::new(),
                schemas: Vec::new(),
                settings_counts: BTreeMap::new(),
            };
        }
        load_sources(&sources, &user)
    })
}

/// The plugins when their load is done; `None` while it runs or before it started.
pub fn loaded() -> Option<&'static LoadedPlugins> {
    BUNDLED.get()
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
/// list and must get one even when nothing loaded the plugins yet (RB-080-51). Never waits.
pub fn ensure_loading(runtime: Handle) {
    if BUNDLED.get().is_some() || !claim(&STARTED) {
        return;
    }
    if let Err(error) = std::thread::Builder::new()
        .name("rb-plugins".to_owned())
        .spawn(move || {
            let _runtime = runtime.enter();
            let _ = bundled();
        })
    {
        STARTED.store(false, Ordering::Release);
        tracing::warn!(%error, "the plugins are loaded on first use");
    }
}

/// A plugin call that failed: the stable code (`plugins.error.*` or the plugin's own key) and
/// its parameters - no English text, no secret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginError {
    pub code: String,
    pub params: Vec<(String, String)>,
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.code)?;
        for (key, value) in &self.params {
            write!(formatter, " {key}={value}")?;
        }
        Ok(())
    }
}

/// The code `plugins.error.not_available` with the plugin's id.
const NOT_AVAILABLE: &str = "plugins.error.not_available";

impl LoadedPlugins {
    /// Calls `tool` of plugin `id` with `args`; the answer as JSON.
    ///
    /// # Errors
    ///
    /// The plugin is not loaded (its refusal, else `plugins.error.not_available`), it is a
    /// project plugin and the workspace is not trusted now (`plugins.error.untrusted_workspace`,
    /// review M2 of Charge 6), or the call failed.
    pub fn call_tool(&self, id: &str, tool: &str, args: &Value) -> Result<Value, PluginError> {
        let Some(entry) = self
            .report
            .loaded
            .iter()
            .find(|entry| entry.plugin.manifest().id == id)
        else {
            return Err(self.refusal(id).unwrap_or_else(|| PluginError {
                code: NOT_AVAILABLE.to_owned(),
                params: vec![("service".to_owned(), id.to_owned())],
            }));
        };
        if entry.source == PluginSource::Project
            && !crate::plugins::services::shared().workspace_trusted()
        {
            return Err(PluginError {
                code: rb_plugin_types::codes::UNTRUSTED_WORKSPACE.to_owned(),
                params: vec![("plugin".to_owned(), id.to_owned())],
            });
        }
        let answer = block_on(entry.plugin.call_tool(tool, &args.to_string()))
            .map_err(PluginError::from)?
            .map_err(|failure| PluginError {
                code: failure.code().unwrap_or(NOT_AVAILABLE).to_owned(),
                params: failure.params.into_iter().collect(),
            })?;
        serde_json::from_str(&answer).map_err(|_| PluginError {
            code: "plugins.error.bad_output".to_owned(),
            params: Vec::new(),
        })
    }

    /// The grants of plugin `id`'s manifest, when it is loaded: the `project-read` globs bound
    /// the files the static source hands it (RB-030-07).
    pub fn grants(&self, id: &str) -> Option<&rb_plugin_host::Grants> {
        self.report
            .loaded
            .iter()
            .find(|entry| entry.plugin.manifest().id == id)
            .map(|entry| &entry.plugin.manifest().grants)
    }

    /// Why plugin `id` did not load, when its package was refused.
    pub fn refusal(&self, id: &str) -> Option<PluginError> {
        self.report
            .refused
            .iter()
            .find(|refused| {
                refused
                    .path
                    .file_stem()
                    .is_some_and(|stem| stem.to_string_lossy().starts_with(id))
            })
            .map(|refused| PluginError {
                code: refused.failure.code().unwrap_or(NOT_AVAILABLE).to_owned(),
                params: refused
                    .failure
                    .params
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect(),
            })
    }
}

impl From<NoRuntime> for PluginError {
    fn from(error: NoRuntime) -> Self {
        Self {
            code: rb_plugin_types::codes::HOST_FAILED.to_owned(),
            params: vec![("detail".to_owned(), error.to_string())],
        }
    }
}

/// The calling thread has no tokio runtime: the CEF UI thread, or a thread started without
/// entering the core's runtime. Waiting there would hang the window (CDP answers on the UI
/// thread), so nothing runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NoRuntime;

impl std::fmt::Display for NoRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("no runtime on this thread: plugin calls run on a blocking thread of the core's runtime")
    }
}

impl std::error::Error for NoRuntime {}

/// Runs `future` to its end from a thread that is no async task - a blocking task of the core's
/// runtime, or a thread that entered its handle - on that runtime. Like
/// [`crate::reload::ReloadService::block_on`], a thread without a runtime gets [`NoRuntime`]
/// instead of a runtime of its own: from the UI thread that would be a deadlock until the
/// plugin's timeout.
///
/// # Errors
///
/// [`NoRuntime`]: no runtime is entered on this thread.
pub(crate) fn block_on<F: std::future::Future>(future: F) -> Result<F::Output, NoRuntime> {
    let handle = Handle::try_current().map_err(|_| NoRuntime)?;
    Ok(handle.block_on(future))
}

#[cfg(test)]
#[path = "plugins_tests.rs"]
mod tests;
