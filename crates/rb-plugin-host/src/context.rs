//! The host functions of one plugin, behind its grants: every capability, path, CDP method
//! ([`crate::cdp_policy`], `cdp.rs`), URL, rate and size is checked here before a service ([`crate::HostServices`]) or the disk is
//! touched. The linker (`link.rs`) calls these and converts the answers to the WIT records; a
//! store credits the time spent here back to the call.
//!
//! New in rBrowser (rCommander's plugins see stream handles, never paths). The checks are the
//! second line: an import the manifest does not grant is refused when the component is compiled
//! (`runtime/imports.rs`) and not linked at all, so a function here that finds its capability
//! missing answers `plugins.error.capability_denied` only to a caller that bypassed both.

use std::{path::PathBuf, sync::Arc, time::Duration};

use rb_plugin_api::{MAX_IO_BYTES, MAX_JSON_CHARS, check::MAX_FINDINGS};
use rb_plugin_types::{
    Capability, Failure, FailureKind, Finding, HttpRequest, HttpResponse, ViewId, ViewInfo, codes,
    paths, rules,
};

use crate::{
    failure::too_large,
    manifest::Grants,
    rate::RateLimiter,
    services::{Caller, HostServices},
};

pub(crate) mod cdp;
pub(crate) mod files;
pub(crate) mod lighthouse_url;
pub(crate) mod trust;

/// The most names one `project.list` answers.
const MAX_LIST_ENTRIES: usize = 4_096;
/// The most `Page.captureScreenshot`s of one plugin a second; more wait for their slot.
pub(crate) const SCREENSHOTS_PER_SECOND: u32 = 10;

/// Everything the host functions of one plugin need, shared by all its stores.
pub struct PluginContext {
    pub(crate) plugin_id: String,
    pub(crate) version: String,
    /// The open project's root; `None` without a project (`project-read` and `out-write` answer
    /// `plugins.error.not_available`).
    pub(crate) project_root: Option<PathBuf>,
    pub(crate) grants: Grants,
    /// The plugin's settings, JSON text by key: what `settings.get` answers.
    pub(crate) settings: Vec<(String, String)>,
    pub(crate) services: Arc<dyn HostServices>,
    pub(crate) rate: RateLimiter,
    /// [`SCREENSHOTS_PER_SECOND`].
    pub(crate) screenshots: RateLimiter,
    pub(crate) max_response_bytes: u64,
    /// How long one host call may take (`limits.timeout_milliseconds`).
    pub(crate) timeout: Duration,
}

impl std::fmt::Debug for PluginContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PluginContext")
            .field("plugin_id", &self.plugin_id)
            .field("project_root", &self.project_root)
            .finish_non_exhaustive()
    }
}

impl PluginContext {
    /// A context for plugin `plugin_id` with `grants`, reaching `services`.
    #[must_use]
    pub fn new(
        plugin_id: &str,
        version: &str,
        grants: Grants,
        services: Arc<dyn HostServices>,
    ) -> Self {
        Self {
            plugin_id: plugin_id.to_owned(),
            version: version.to_owned(),
            project_root: None,
            rate: RateLimiter::new(grants.http_rate_per_second),
            screenshots: RateLimiter::new(SCREENSHOTS_PER_SECOND),
            grants,
            settings: Vec::new(),
            services,
            max_response_bytes: crate::limits::DEFAULT_MAX_RESPONSE_BYTES,
            timeout: Duration::from_millis(crate::limits::DEFAULT_TIMEOUT_MILLISECONDS),
        }
    }

    /// How long one host call of the plugin may take: its `limits.timeout_milliseconds`.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    fn caller(&self) -> Caller<'_> {
        Caller {
            plugin_id: &self.plugin_id,
            timeout: self.timeout,
        }
    }

    /// The open project, for `project-read` and `out-write`.
    #[must_use]
    pub fn with_project(mut self, root: Option<PathBuf>) -> Self {
        self.project_root = root;
        self
    }

    /// The plugin's settings, JSON text by key.
    #[must_use]
    pub fn with_settings(mut self, settings: Vec<(String, String)>) -> Self {
        self.settings = settings;
        self
    }

    /// The largest HTTP answer the plugin may read (`limits.max_response_bytes`).
    #[must_use]
    pub fn with_max_response_bytes(mut self, bytes: u64) -> Self {
        self.max_response_bytes = bytes;
        self
    }

    fn require(&self, capability: Capability) -> Result<(), Failure> {
        if self.grants.has(capability) {
            Ok(())
        } else {
            Err(capability.denied())
        }
    }

    /// The project given at load, else the one the services have open now.
    fn root(&self) -> Result<PathBuf, Failure> {
        self.project_root
            .clone()
            .or_else(|| self.services.project_root())
            .ok_or_else(|| crate::services::not_available("project"))
    }

    /// `settings.get`: the value in effect now (the services), else the one at load.
    pub(crate) fn setting(&self, key: &str) -> Option<String> {
        self.services.setting(&self.plugin_id, key).or_else(|| {
            self.settings
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value.clone())
        })
    }

    /// `views.list`: nothing without `views`.
    pub(crate) fn views(&self) -> Vec<ViewInfo> {
        if self.grants.has(Capability::Views) {
            self.services.views()
        } else {
            Vec::new()
        }
    }

    /// `views.active`.
    pub(crate) fn active_view(&self) -> Option<ViewId> {
        self.grants
            .has(Capability::Views)
            .then(|| self.services.active_view())
            .flatten()
    }

    /// `evaluate.evaluate`.
    pub(crate) async fn evaluate(&self, view: ViewId, script: &str) -> Result<String, Failure> {
        self.require(Capability::Evaluate)?;
        if script.len() > MAX_JSON_CHARS {
            return Err(too_large(MAX_JSON_CHARS));
        }
        bounded_json(self.services.evaluate(&self.caller(), view, script).await?)
    }

    /// `http.fetch`: http and https only, at the manifest's rate, the answer bounded.
    pub(crate) async fn fetch(&self, request: HttpRequest) -> Result<HttpResponse, Failure> {
        self.require(Capability::Http)?;
        rules::check_http_url(&request.url)?;
        if request.body.len() > MAX_IO_BYTES {
            return Err(too_large(MAX_IO_BYTES));
        }
        let wait = self.rate.reserve();
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
        let answer = self
            .services
            .fetch(request, self.max_response_bytes)
            .await?;
        if answer.body.len() as u64 > self.max_response_bytes {
            return Err(too_large(
                usize::try_from(self.max_response_bytes).unwrap_or(usize::MAX),
            ));
        }
        Ok(answer)
    }

    /// `project.read`: `.rbrowser/` (never `config.local.json`, the trust-protected folders only
    /// under workspace trust, [`trust`]) and the manifest's globs.
    pub(crate) async fn project_read(&self, path: &str) -> Result<Vec<u8>, Failure> {
        self.require(Capability::ProjectRead)?;
        let root = self.root()?;
        let segments = paths::relative_segments(path).map_err(|refusal| refusal.failure(path))?;
        trust::check(&segments, self.services.workspace_trusted(), path)?;
        self.check_readable(path, &segments)?;
        files::read(&root, &segments, path).await
    }

    /// `project.list`: the names in a folder `project.read` could read from; `dir` empty is the
    /// root (RB-110-13), where `.rbrowser/` is always listed.
    pub(crate) async fn project_list(&self, dir: &str) -> Result<Vec<String>, Failure> {
        self.require(Capability::ProjectRead)?;
        let root = self.root()?;
        let segments = if dir.is_empty() {
            Vec::new()
        } else {
            paths::relative_segments(dir).map_err(|refusal| refusal.failure(dir))?
        };
        trust::check(&segments, self.services.workspace_trusted(), dir)?;
        let in_config = segments
            .first()
            .is_some_and(|first| paths::is_config_dir(first));
        let entries = files::list(&root, &segments, dir).await?;
        let prefix = segments.join("/");
        let names: Vec<String> = entries
            .into_iter()
            .filter(|(name, is_dir)| {
                let relative = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{prefix}/{name}")
                };
                let child: Vec<&str> = relative.split('/').collect();
                if in_config {
                    !paths::is_local_config(&child)
                } else if *is_dir {
                    (segments.is_empty() && paths::is_config_dir(name))
                        || self.glob_reaches(&relative)
                } else {
                    self.grants.glob_matches(&relative)
                }
            })
            .map(|(name, is_dir)| if is_dir { format!("{name}/") } else { name })
            .take(MAX_LIST_ENTRIES)
            .collect();
        Ok(names)
    }

    /// `out.write`: only below `.rbrowser/out/<plugin-id>/`.
    pub(crate) async fn out_write(&self, path: &str, bytes: &[u8]) -> Result<(), Failure> {
        self.require(Capability::OutWrite)?;
        let root = self.root()?;
        let segments = paths::relative_segments(path).map_err(|refusal| refusal.failure(path))?;
        if bytes.len() > MAX_IO_BYTES {
            return Err(too_large(MAX_IO_BYTES));
        }
        let mut full = vec![paths::CONFIG_DIR, paths::OUT_DIR, self.plugin_id.as_str()];
        full.extend(segments);
        files::write(&root, &full, path, bytes).await
    }

    /// `project-write.write`: a file a write glob and a read glob name, never in `.git/` or
    /// `.rbrowser/`, only in a trusted workspace, replaced atomically and only while it still
    /// holds `expected` (`None`: while it does not exist) - otherwise
    /// `plugins.error.write_conflict`.
    pub(crate) async fn project_write(
        &self,
        path: &str,
        bytes: &[u8],
        expected: Option<&[u8]>,
    ) -> Result<(), Failure> {
        self.require(Capability::ProjectWrite)?;
        let root = self.root()?;
        let segments = paths::relative_segments(path).map_err(|refusal| refusal.failure(path))?;
        trust::require(self.services.workspace_trusted(), path)?;
        // A file it may not read is no file it may write: the `expected` comparison would tell
        // its content (security review RB-110-13).
        let relative = segments.join("/");
        if paths::is_write_protected(&segments)
            || !self.grants.write_glob_matches(&relative)
            || !self.grants.glob_matches(&relative)
        {
            return Err(
                Failure::coded(FailureKind::AccessDenied, codes::PATH_NOT_WRITABLE)
                    .with_param("path", path.chars().take(200).collect::<String>()),
            );
        }
        if bytes.len() > MAX_IO_BYTES || expected.is_some_and(|old| old.len() > MAX_IO_BYTES) {
            return Err(too_large(MAX_IO_BYTES));
        }
        files::replace(&root, &segments, path, bytes, expected).await
    }

    /// `host.report-progress`: `done` held to `total`, no capability needed.
    pub(crate) fn report_progress(&self, done: u32, total: u32) {
        self.services
            .report_progress(&self.plugin_id, done.min(total), total);
    }

    /// `findings.report`.
    pub(crate) fn report_findings(&self, items: Vec<Finding>) -> Result<(), Failure> {
        self.require(Capability::Findings)?;
        if items.len() > MAX_FINDINGS {
            return Err(too_large(MAX_FINDINGS));
        }
        // The same rules as a check run's answer: rule ids, the plugin's own message keys, sizes.
        let prefix = format!("plugin.{}.", self.plugin_id);
        let items = crate::plugin::answers::findings(items, &prefix).map_err(|_| {
            Failure::coded(FailureKind::BadData, codes::INVALID).with_param("reason", "finding")
        })?;
        self.services.report_findings(&self.plugin_id, items)
    }

    /// `sidecar.call`.
    pub(crate) async fn sidecar_call(&self, method: &str, params: &str) -> Result<String, Failure> {
        self.require(Capability::Sidecar)?;
        if !self.grants.sidecar_allows(method) {
            return Err(Failure::coded(FailureKind::AccessDenied, codes::DENIED)
                .with_param("method", method.chars().take(64).collect::<String>()));
        }
        if params.len() > MAX_JSON_CHARS {
            return Err(too_large(MAX_JSON_CHARS));
        }
        if method == lighthouse_url::START && !self.services.lighthouse_any_url() {
            lighthouse_url::check(params, &self.services.views())?;
        }
        bounded_json(
            self.services
                .sidecar_call(&self.caller(), method, params)
                .await?,
        )
    }

    fn check_readable(&self, path: &str, segments: &[&str]) -> Result<(), Failure> {
        let relative = segments.join("/");
        let granted = if paths::in_config_dir(segments) {
            !paths::is_local_config(segments)
        } else {
            self.grants.glob_matches(&relative)
        };
        if granted {
            Ok(())
        } else {
            Err(
                Failure::coded(FailureKind::AccessDenied, codes::PATH_NOT_GRANTED)
                    .with_param("path", path.chars().take(200).collect::<String>()),
            )
        }
    }

    /// Whether a glob may match something below the folder `relative`: its literal start (the
    /// part before the first wildcard) and the folder lie on one path.
    fn glob_reaches(&self, relative: &str) -> bool {
        self.grants.project_globs.iter().any(|glob| {
            let literal: String = glob
                .chars()
                .take_while(|c| !matches!(c, '*' | '?' | '[' | '{'))
                .collect();
            let folder = format!("{relative}/");
            literal.starts_with(&folder) || folder.starts_with(&literal)
        })
    }
}

fn bounded_json(answer: String) -> Result<String, Failure> {
    if answer.len() > MAX_JSON_CHARS {
        Err(too_large(MAX_JSON_CHARS))
    } else {
        Ok(answer)
    }
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "context_services_tests.rs"]
mod services_tests;

#[cfg(test)]
#[path = "context_write_tests.rs"]
mod write_tests;
