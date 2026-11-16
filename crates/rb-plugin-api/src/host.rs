//! What a plugin may ask of rBrowser: every import interface of the contract, as one native trait.
//!
//! Adapted from rCommander's per-world host traits (`ViewerHost`, `CompareHost`): rBrowser's worlds
//! share their imports, and what a plugin may call is decided by its manifest's capabilities, not
//! by its world - so one trait, and every function behind a capability answers
//! `plugins.error.capability_denied` when the capability is not granted. The real host refuses
//! such a component before it runs; the native test host ([`crate::test_support::MemoryHost`])
//! answers the failure, so a plugin's own tests see what it would be denied.

use crate::{Failure, Finding, HttpRequest, HttpResponse, ViewId, ViewInfo};

/// The host interfaces of `rbrowser:plugin@0.1.0`.
pub trait PluginHost {
    /// `host.log`: a line in rBrowser's log, capped by the log budget.
    fn log(&mut self, level: &str, message: &str);

    /// `host.now-unix-seconds`.
    fn now_unix_seconds(&mut self) -> u64;

    /// `host.report-progress`: `done` of `total` steps of a long call. Nothing by default, so a
    /// guest that never reports needs no implementation.
    fn report_progress(&mut self, done: u32, total: u32) {
        let _ = (done, total);
    }

    /// `settings.get`: the plugin's own setting `key` as JSON text; `None` when undeclared.
    fn setting(&mut self, key: &str) -> Option<String>;

    /// `views.list` (capability `views`).
    fn views(&mut self) -> Result<Vec<ViewInfo>, Failure>;

    /// `views.active` (capability `views`).
    fn active_view(&mut self) -> Result<Option<ViewId>, Failure>;

    /// `evaluate.evaluate` (capability `evaluate`): the result as JSON text.
    fn evaluate(&mut self, view: ViewId, script: &str) -> Result<String, Failure>;

    /// `cdp.send` (capability `cdp`, the manifest's domains only).
    fn cdp_send(&mut self, view: ViewId, method: &str, params: &str) -> Result<String, Failure>;

    /// `http.fetch` (capability `http`).
    fn fetch(&mut self, request: &HttpRequest) -> Result<HttpResponse, Failure>;

    /// `project.read` (capability `project-read`): a project-relative path.
    fn project_read(&mut self, path: &str) -> Result<Vec<u8>, Failure>;

    /// `project.list` (capability `project-read`): the names in a project-relative folder.
    fn project_list(&mut self, dir: &str) -> Result<Vec<String>, Failure>;

    /// `project-write.write` (capability `project-write`): replaces the project file `path` with
    /// `bytes` when it still holds `expected` (`None`: when it does not exist yet), atomically;
    /// otherwise `plugins.error.write_conflict` and nothing written. Refused by default, so a
    /// host built before the capability needs no implementation.
    fn project_write(
        &mut self,
        path: &str,
        bytes: &[u8],
        expected: Option<&[u8]>,
    ) -> Result<(), Failure> {
        let _ = (path, bytes, expected);
        Err(crate::Capability::ProjectWrite.denied())
    }

    /// `out.write` (capability `out-write`): a path relative to `.rbrowser/out/<plugin-id>/`.
    fn out_write(&mut self, path: &str, bytes: &[u8]) -> Result<(), Failure>;

    /// `findings.report` (capability `findings`).
    fn report_findings(&mut self, items: &[Finding]) -> Result<(), Failure>;

    /// `sidecar.call` (capability `sidecar`): parameters and answer as JSON text.
    fn sidecar_call(&mut self, method: &str, params: &str) -> Result<String, Failure>;
}
