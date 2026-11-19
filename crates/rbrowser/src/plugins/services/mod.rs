//! [`AppServices`]: the host services behind the plugins' WIT imports in the app (Charge 6,
//! completing RB-060-12; `docs/plugins.md`, "Capabilities and rights"). `rb_plugin_host` checks
//! every grant, path, CDP domain, URL, rate and size before a call reaches here; this module
//! only does the work, for the window the runtime started in ([`AppServices::bind`]):
//!
//! - `views`, `evaluate`, `cdp` over the window's views ([`page`]); a lasting emulation a plugin
//!   set is undone when it is disabled or loses the workspace trust ([`resets`]);
//! - `http` through a transport that resolves every name itself and connects only to public
//!   addresses, or to a trusted project's own origins ([`http`], [`net`]);
//! - `project-read` and `out-write` below the open project ([`AppServices::project_root`]); the
//!   trust-protected folders only while the shell's workspace trust holds;
//! - `settings.get` by the scopes the plugin's schema allows - `config.local.json`, `config.json`,
//!   the user settings - resolved once per change ([`settings`]);
//! - `findings.report` as a findings run of the plugin's own (`crate::checks::reported`);
//! - `sidecar` through the app's Node sidecar, only the methods the manifest lists, each call
//!   naming its plugin ([`with_caller`]).
//!
//! A project plugin reaches none of them while the workspace is not trusted (review M2 of
//! Charge 6): its calls are refused before they start (`super::runtime::may_run`), and a call
//! still running when the trust goes gets `plugins.error.untrusted_workspace` from every service
//! that names its caller.
//!
//! One instance per process: the plugins load once, the window binds when its runtime starts.

pub mod http;
pub mod net;
pub mod page;
pub mod resets;
pub mod settings;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, PoisonError, RwLock, Weak};

use async_trait::async_trait;
use rb_engine::ViewId;
use rb_plugin_host::{Caller, HostServices, not_available};
use rb_plugin_types::{Failure, FailureKind, Finding, HttpRequest, HttpResponse, ViewInfo, codes};
use serde_json::Value;

use crate::control::Core;
use http::LocalOrigins;
use net::Origin;

/// The services of this process.
static SHARED: OnceLock<Arc<AppServices>> = OnceLock::new();

/// The services every plugin of this process reaches.
pub fn shared() -> Arc<AppServices> {
    Arc::clone(SHARED.get_or_init(|| Arc::new(AppServices::new())))
}

/// The app's [`HostServices`].
pub struct AppServices {
    window: RwLock<Weak<Core>>,
    agent: ureq::Agent,
    local: LocalOrigins,
    /// The settings schemas of the loaded plugins, by plugin id.
    schemas: RwLock<Vec<(String, Value)>>,
    /// The ids of the loaded project plugins.
    project_plugins: RwLock<BTreeSet<String>>,
    /// Every plugin's settings values, resolved once per change.
    values: settings::SettingsCache,
    /// The lasting CDP state each plugin set on the views.
    resets: resets::CdpResets,
}

impl AppServices {
    fn new() -> Self {
        let local = LocalOrigins::default();
        Self {
            window: RwLock::new(Weak::new()),
            agent: http::agent(Arc::clone(&local)),
            local,
            schemas: RwLock::new(Vec::new()),
            project_plugins: RwLock::new(BTreeSet::new()),
            values: settings::SettingsCache::default(),
            resets: resets::CdpResets::default(),
        }
    }

    /// The loaded project plugins, by id: they reach a service only under workspace trust.
    pub fn set_project_plugins(&self, ids: BTreeSet<String>) {
        if let Ok(mut held) = self.project_plugins.write() {
            *held = ids;
        }
    }

    /// `plugins.error.untrusted_workspace` for a project plugin while the workspace is not
    /// trusted.
    fn admit(&self, plugin_id: &str) -> Result<(), Failure> {
        let project = self
            .project_plugins
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(plugin_id);
        if project && !self.workspace_trusted() {
            return Err(
                Failure::coded(FailureKind::AccessDenied, codes::UNTRUSTED_WORKSPACE)
                    .with_param("plugin", plugin_id),
            );
        }
        Ok(())
    }

    /// The plugins serve `core`'s window from now on.
    pub fn bind(&self, core: &Arc<Core>) {
        if let Ok(mut window) = self.window.write() {
            *window = Arc::downgrade(core);
        }
        self.project_changed(core);
    }

    /// The project, its config or its trust changed: the origins of a trusted project are the
    /// local ones a plugin may fetch ([`local_origins`]).
    pub fn project_changed(&self, core: &Core) {
        let host = core.host();
        let origins = local_origins(&host.config(), host.shell_state().trusted);
        if let Ok(mut local) = self.local.write() {
            *local = origins;
        }
        self.settings_changed();
    }

    /// The user settings or the project config changed: the plugins' settings values are
    /// resolved again at their next use.
    pub fn settings_changed(&self) {
        self.values.invalidate();
    }

    /// The local origins in effect.
    #[cfg(test)]
    pub fn local_now(&self) -> Vec<Origin> {
        self.local
            .read()
            .map(|local| local.clone())
            .unwrap_or_default()
    }

    /// Plugins `plugin_ids` stopped (disabled, or a project's without trust): the lasting CDP
    /// state each set on a view is undone ([`resets`]); a view gone since is skipped.
    pub fn reset_cdp<'a>(&self, plugin_ids: impl IntoIterator<Item = &'a str>) {
        let undo: Vec<_> = plugin_ids
            .into_iter()
            .flat_map(|plugin| self.resets.take(plugin))
            .collect();
        let Some(core) = self.core() else {
            return;
        };
        if undo.is_empty() {
            return;
        }
        let host = Arc::downgrade(core.host());
        core.runtime().spawn(async move {
            for (view, method, params) in undo {
                let Some(host) = host.upgrade() else {
                    return;
                };
                let params = params.to_string();
                let timeout = std::time::Duration::from_secs(5);
                if let Err(failure) = page::cdp(&*host, view, method, &params, timeout).await {
                    tracing::debug!(%view, method, %failure, "a plugin's emulation was not undone");
                }
            }
        });
    }

    /// The settings schemas the loaded plugins declare.
    pub fn set_schemas(&self, schemas: Vec<(String, Value)>) {
        if let Ok(mut held) = self.schemas.write() {
            *held = schemas;
        }
        self.settings_changed();
    }

    /// The values of plugin `plugin_id`'s own settings keys, `<plugin-id>.<path>` to JSON.
    pub fn plugin_values(&self, plugin_id: &str) -> serde_json::Map<String, Value> {
        self.values().get(plugin_id).cloned().unwrap_or_default()
    }

    /// Every plugin's values: the cache's, or resolved now over the documents in effect.
    fn values(&self) -> Arc<settings::PluginValues> {
        self.values.get_or_build(|| {
            let layers = self.layers();
            let schemas = self
                .schemas
                .read()
                .map(|schemas| schemas.clone())
                .unwrap_or_default();
            schemas
                .iter()
                .map(|(id, schema)| (id.clone(), settings::plugin_values(id, schema, &layers)))
                .collect()
        })
    }

    fn core(&self) -> Option<Arc<Core>> {
        self.window.read().ok().and_then(|window| window.upgrade())
    }

    fn core_or(&self, service: &str) -> Result<Arc<Core>, Failure> {
        self.core().ok_or_else(|| not_available(service))
    }

    /// The documents the settings resolve over: the user settings in effect (the window's,
    /// else the file's) and the open project's two config files as stored.
    fn layers(&self) -> settings::Layers {
        let core = self.core();
        let user = core.as_ref().map_or_else(
            || crate::control::settings::load_user().settings,
            |core| core.user_settings(),
        );
        let root = core.as_ref().and_then(|core| core.host().project_root());
        let read =
            |file: rb_config::ConfigFile| rb_config::read_json(&file.path(root.as_deref()?)).ok();
        settings::Layers {
            user: serde_json::to_value(user).unwrap_or(Value::Null),
            project: read(rb_config::ConfigFile::Project),
            local: read(rb_config::ConfigFile::Local),
        }
    }
}

/// The origins a plugin may fetch on a non-public address: the project's own
/// ([`project_origins`]) once the workspace is trusted, none before - an untrusted project's
/// config could name any local service (review finding L2 of Charge 6).
#[must_use]
pub fn local_origins(config: &rb_config::ProjectConfig, trusted: bool) -> Vec<Origin> {
    if trusted {
        project_origins(config)
    } else {
        Vec::new()
    }
}

/// The origins of the project's `startUrl` and environments.
#[must_use]
pub fn project_origins(config: &rb_config::ProjectConfig) -> Vec<Origin> {
    config
        .start_url
        .iter()
        .chain(
            config
                .environments
                .iter()
                .map(|environment| &environment.url),
        )
        .filter_map(|url| Origin::of(url))
        .collect()
}

/// The key of a sidecar call's parameters that names the calling plugin.
pub const CALLER_KEY: &str = "$caller";

/// A plugin's sidecar `params` with its id under [`CALLER_KEY`], over anything it put there, so
/// the sidecar can keep each plugin to its own jobs (review finding L4 of Charge 7); none for
/// parameters that are no object, which could not carry it (review finding L4 of Charge 8).
#[must_use]
pub fn with_caller(params: Value, plugin_id: &str) -> Option<Value> {
    match params {
        Value::Object(mut map) => {
            map.insert(CALLER_KEY.to_owned(), Value::String(plugin_id.to_owned()));
            Some(Value::Object(map))
        }
        _ => None,
    }
}

#[async_trait]
impl HostServices for AppServices {
    fn views(&self) -> Vec<ViewInfo> {
        self.core()
            .map(|core| {
                page::Views::views(&**core.host())
                    .iter()
                    .map(page::plugin_view)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn active_view(&self) -> Option<u32> {
        self.core()
            // The active view, else the first (RB-090-82): before a person clicked a view none
            // is active, and a plugin's call on "the active view" failed.
            .and_then(|core| core.host().active_or_first().ok())
            .map(|view| view.0)
    }

    fn project_root(&self) -> Option<PathBuf> {
        self.core().and_then(|core| core.host().project_root())
    }

    fn workspace_trusted(&self) -> bool {
        self.core()
            .is_some_and(|core| core.host().shell_state().trusted)
    }

    fn lighthouse_any_url(&self) -> bool {
        // Without the window, the limit holds (RB-090-21).
        self.core()
            .is_some_and(|core| core.user_settings().plugins.lighthouse_any_url)
    }

    fn setting(&self, plugin_id: &str, key: &str) -> Option<String> {
        self.admit(plugin_id).ok()?;
        if key == rb_plugin_types::RULE_LEVELS_SETTING {
            // The levels of the plugin's own rules, as the findings run applies them (RB-091-06).
            let core = self.core()?;
            let project = core.host().config().checks.findings;
            return Some(crate::checks::run::rule_levels(
                &project,
                &core.user_settings().checks,
                plugin_id,
            ));
        }
        settings::plugin_setting(plugin_id, self.values().get(plugin_id)?, key)
    }

    async fn evaluate(
        &self,
        caller: &Caller<'_>,
        view: u32,
        script: &str,
    ) -> Result<String, Failure> {
        self.admit(caller.plugin_id)?;
        let core = self.core_or("evaluate")?;
        let world = page::world_of(caller.plugin_id);
        page::evaluate(&**core.host(), ViewId(view), &world, script, caller.timeout).await
    }

    async fn cdp_send(
        &self,
        caller: &Caller<'_>,
        view: u32,
        method: &str,
        params: &str,
    ) -> Result<String, Failure> {
        self.admit(caller.plugin_id)?;
        let core = self.core_or("cdp")?;
        let answer =
            page::cdp(&**core.host(), ViewId(view), method, params, caller.timeout).await?;
        if let Ok(sent) = serde_json::from_str::<Value>(params) {
            self.resets.note(caller.plugin_id, view, method, &sent);
        }
        Ok(answer)
    }

    async fn fetch(
        &self,
        request: HttpRequest,
        max_response_bytes: u64,
    ) -> Result<HttpResponse, Failure> {
        let agent = self.agent.clone();
        tokio::task::spawn_blocking(move || {
            http::fetch_blocking(&agent, &request, max_response_bytes)
        })
        .await
        .map_err(|_| Failure::coded(FailureKind::Io, codes::HTTP_FAILED))?
    }

    async fn sidecar_call(
        &self,
        caller: &Caller<'_>,
        method: &str,
        params: &str,
    ) -> Result<String, Failure> {
        self.admit(caller.plugin_id)?;
        let invalid = || Failure::coded(FailureKind::BadData, codes::INVALID);
        let params: Value = serde_json::from_str(params).map_err(|_| invalid())?;
        let params = with_caller(params, caller.plugin_id).ok_or_else(invalid)?;
        let sidecar = crate::sidecar::shared().ok_or_else(|| not_available("sidecar"))?;
        let call = sidecar.request(method, params);
        match tokio::time::timeout(caller.timeout, call).await {
            Ok(Ok(answer)) => Ok(answer.to_string()),
            Ok(Err(error)) => Err(Failure::coded(FailureKind::Other, codes::HOST_FAILED)
                .with_param(
                    "reason",
                    error.to_string().chars().take(200).collect::<String>(),
                )),
            Err(_) => Err(Failure::coded(FailureKind::Other, codes::HOST_FAILED)
                .with_param("reason", "timeout")),
        }
    }

    fn report_findings(&self, plugin_id: &str, findings: Vec<Finding>) -> Result<(), Failure> {
        self.admit(plugin_id)?;
        let core = self.core_or("findings")?;
        crate::checks::reported::publish(&core, plugin_id, findings);
        Ok(())
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
