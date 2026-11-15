//! What the host functions reach outside the sandbox: the views, CDP, the network, the sidecar,
//! the findings list - one trait, implemented by the application.
//!
//! New in rBrowser. The host checks every capability, path, CDP domain, URL, rate and size
//! before it calls a service ([`crate::context`]); a service only does the work. The wiring is the
//! owning jobs': views, `evaluate` and CDP over `rb_engine::ViewHost` (RB-010-04), the sidecar
//! (RB-040-03), the findings list (RB-060-01), the HTTP transport (the first plugin that fetches,
//! RB-060-08). Until a service is wired its default answers `plugins.error.not_available`.
//! The app's services are `rbrowser`'s `plugins::services::AppServices` (Charge 6).

use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use rb_plugin_types::{
    Failure, FailureKind, Finding, HttpRequest, HttpResponse, ViewId, ViewInfo, codes,
};

/// `plugins.error.not_available`, naming the service.
#[must_use]
pub fn not_available(service: &str) -> Failure {
    Failure::coded(FailureKind::NotSupported, codes::NOT_AVAILABLE).with_param("service", service)
}

/// Who calls a service: the plugin, and how long one host call of it may take (the plugin's
/// `limits.timeout_milliseconds`). The plugin id names its isolated world
/// (`rbplugin-<id>`) and its own settings; it is the host's, never the plugin's word.
#[derive(Clone, Copy, Debug)]
pub struct Caller<'a> {
    pub plugin_id: &'a str,
    pub timeout: Duration,
}

/// The application's side of the host functions. Every method has a default that answers
/// [`not_available`], so a process wires only what it has.
#[async_trait]
pub trait HostServices: Send + Sync {
    /// The views, for `views.list`.
    fn views(&self) -> Vec<ViewInfo> {
        Vec::new()
    }

    /// The active view, for `views.active`.
    fn active_view(&self) -> Option<ViewId> {
        None
    }

    /// The open project's root, when the plugin was loaded without one (the app opens its
    /// project after the plugins load): `project-read` and `out-write` work below it.
    fn project_root(&self) -> Option<PathBuf> {
        None
    }

    /// Whether the person trusts the open workspace in its current form (RB-010-09): the
    /// trust-protected folders of `.rbrowser/` are read only then (`context::trust`). `false`
    /// without a project or a trust service.
    fn workspace_trusted(&self) -> bool {
        false
    }

    /// Whether a person let plugins run Lighthouse on any address (`plugins.lighthouseAnyUrl`,
    /// protected, RB-090-21); `false` holds a run to the sites of the views
    /// (`context::lighthouse_url`).
    fn lighthouse_any_url(&self) -> bool {
        false
    }

    /// The effective value of the caller's own settings key `key` (the property path below
    /// `<plugin-id>.`), JSON text; `None` for an undeclared key or no settings service.
    fn setting(&self, plugin_id: &str, key: &str) -> Option<String> {
        let _ = (plugin_id, key);
        None
    }

    /// Runs `script` in the caller's isolated world (`rbplugin-<id>`) of `view`; the result as
    /// JSON text.
    async fn evaluate(
        &self,
        caller: &Caller<'_>,
        view: ViewId,
        script: &str,
    ) -> Result<String, Failure> {
        let _ = (caller, view, script);
        Err(not_available("evaluate"))
    }

    /// One CDP command on `view`'s target; the domain is checked already.
    async fn cdp_send(
        &self,
        caller: &Caller<'_>,
        view: ViewId,
        method: &str,
        params: &str,
    ) -> Result<String, Failure> {
        let _ = (caller, view, method, params);
        Err(not_available("cdp"))
    }

    /// One HTTP request, reading at most `max_response_bytes` of the answer; a redirect is
    /// answered, never followed. The URL and the rate are checked already.
    async fn fetch(
        &self,
        request: HttpRequest,
        max_response_bytes: u64,
    ) -> Result<HttpResponse, Failure> {
        let _ = (request, max_response_bytes);
        Err(not_available("http"))
    }

    /// One sidecar call (`lighthouse.run`, ...); parameters and answer JSON text. The method is
    /// one the caller's manifest lists already.
    async fn sidecar_call(
        &self,
        caller: &Caller<'_>,
        method: &str,
        params: &str,
    ) -> Result<String, Failure> {
        let _ = (caller, method, params);
        Err(not_available("sidecar"))
    }

    /// How far plugin `plugin_id`'s current call is (`host.report-progress`); nothing by default.
    fn report_progress(&self, plugin_id: &str, done: u32, total: u32) {
        let _ = (plugin_id, done, total);
    }

    /// Findings plugin `plugin_id` reports, for the findings list.
    fn report_findings(&self, plugin_id: &str, findings: Vec<Finding>) -> Result<(), Failure> {
        let _ = (plugin_id, findings);
        Err(not_available("findings"))
    }
}

/// No service wired: every call answers `plugins.error.not_available`.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoServices;

impl HostServices for NoServices {}
