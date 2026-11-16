//! An in-memory [`PluginHost`] for a plugin's native tests.
//!
//! Copied and adapted from rCommander's `crates/rc-plugin-api/src/test_support.rs` (the scripted
//! host the plugins' native suites share). It holds the host's promises a plugin may rely on, so a
//! test against it finds what the real host would refuse: every capability checked against the
//! grants ([`MemoryHost::grant`]), the path rule of `project-read`, `project-write` and
//! `out-write` ([`crate::paths`]), workspace trust and the write conflict of `project-write`, the
//! CDP domain list, the URL rule of `http`, the size limits.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Capability, Failure, FailureKind, Finding, HttpRequest, HttpResponse, MAX_IO_BYTES, PluginHost,
    ViewId, ViewInfo, codes, paths, rules,
};

/// Views, files, answers and everything a plugin did, in memory.
#[derive(Debug, Default)]
pub struct MemoryHost {
    granted: BTreeSet<Capability>,
    /// The manifest's `cdp.domains`.
    pub cdp_domains: Vec<String>,
    /// Project files by project-relative path (`/`-separated).
    pub project: BTreeMap<String, Vec<u8>>,
    /// Project-relative paths outside `.rbrowser/` the manifest's globs cover.
    pub readable: BTreeSet<String>,
    /// Project-relative paths the manifest's `project-write` globs cover; written only when
    /// [`MemoryHost::readable`] holds them too, as the host requires a matching read glob.
    pub writable: BTreeSet<String>,
    /// Whether the person trusts the workspace: `project-write` writes only then.
    pub trusted: bool,
    /// What `out.write` wrote, by path relative to the plugin's out folder.
    pub out: BTreeMap<String, Vec<u8>>,
    pub views: Vec<ViewInfo>,
    pub active: Option<ViewId>,
    /// What `evaluate` answers.
    pub evaluate_answer: String,
    /// What `cdp.send` answers.
    pub cdp_answer: String,
    /// What `sidecar.call` answers.
    pub sidecar_answer: String,
    /// What `http.fetch` answers, by URL; an unknown URL answers 404.
    pub responses: BTreeMap<String, HttpResponse>,
    /// The plugin's settings, as JSON text by key.
    pub settings: BTreeMap<String, String>,
    pub now: u64,
    /// Every line logged, as `(level, message)`.
    pub logs: Vec<(String, String)>,
    /// Every script evaluated, as `(view, script)`.
    pub evaluated: Vec<(ViewId, String)>,
    /// Every CDP command sent, as `(view, method, params)`.
    pub cdp_sent: Vec<(ViewId, String, String)>,
    /// Every HTTP request fetched.
    pub fetched: Vec<HttpRequest>,
    /// Every finding reported.
    pub findings: Vec<Finding>,
    /// Every progress reported, as `(done, total)`.
    pub progress: Vec<(u32, u32)>,
}

impl MemoryHost {
    /// A host granting nothing: only `log`, the clock and the settings answer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            evaluate_answer: "null".to_owned(),
            cdp_answer: "{}".to_owned(),
            sidecar_answer: "null".to_owned(),
            ..Self::default()
        }
    }

    /// Grants `capability`, as a manifest's `[capabilities]` would.
    #[must_use]
    pub fn grant(mut self, capability: Capability) -> Self {
        self.granted.insert(capability);
        self
    }

    /// Adds a project file.
    #[must_use]
    pub fn with_file(mut self, path: &str, bytes: &[u8]) -> Self {
        self.project.insert(path.to_owned(), bytes.to_vec());
        self
    }

    fn require(&self, capability: Capability) -> Result<(), Failure> {
        if self.granted.contains(&capability) {
            Ok(())
        } else {
            Err(capability.denied())
        }
    }
}

fn too_large(limit: usize) -> Failure {
    Failure::coded(FailureKind::Limit, codes::TOO_LARGE).with_param("limit", limit)
}

fn not_found(path: &str) -> Failure {
    Failure::coded(FailureKind::NotFound, codes::NOT_FOUND).with_param("path", path)
}

impl PluginHost for MemoryHost {
    fn log(&mut self, level: &str, message: &str) {
        self.logs.push((level.to_owned(), message.to_owned()));
    }

    fn now_unix_seconds(&mut self) -> u64 {
        self.now
    }

    fn report_progress(&mut self, done: u32, total: u32) {
        self.progress.push((done, total));
    }

    fn setting(&mut self, key: &str) -> Option<String> {
        self.settings.get(key).cloned()
    }

    fn views(&mut self) -> Result<Vec<ViewInfo>, Failure> {
        self.require(Capability::Views)?;
        Ok(self.views.clone())
    }

    fn active_view(&mut self) -> Result<Option<ViewId>, Failure> {
        self.require(Capability::Views)?;
        Ok(self.active)
    }

    fn evaluate(&mut self, view: ViewId, script: &str) -> Result<String, Failure> {
        self.require(Capability::Evaluate)?;
        self.evaluated.push((view, script.to_owned()));
        Ok(self.evaluate_answer.clone())
    }

    fn cdp_send(&mut self, view: ViewId, method: &str, params: &str) -> Result<String, Failure> {
        self.require(Capability::Cdp)?;
        rules::check_cdp_method(method, &self.cdp_domains)?;
        self.cdp_sent
            .push((view, method.to_owned(), params.to_owned()));
        Ok(self.cdp_answer.clone())
    }

    fn fetch(&mut self, request: &HttpRequest) -> Result<HttpResponse, Failure> {
        self.require(Capability::Http)?;
        rules::check_http_url(&request.url)?;
        self.fetched.push(request.clone());
        Ok(self
            .responses
            .get(&request.url)
            .cloned()
            .unwrap_or(HttpResponse {
                status: 404,
                ..HttpResponse::default()
            }))
    }

    fn project_read(&mut self, path: &str) -> Result<Vec<u8>, Failure> {
        self.require(Capability::ProjectRead)?;
        let segments = paths::relative_segments(path).map_err(|refusal| refusal.failure(path))?;
        let key = segments.join("/");
        let granted = (paths::in_config_dir(&segments) && !paths::is_local_config(&segments))
            || self.readable.contains(&key);
        if !granted {
            return Err(
                Failure::coded(FailureKind::AccessDenied, codes::PATH_NOT_GRANTED)
                    .with_param("path", path),
            );
        }
        let bytes = self.project.get(&key).ok_or_else(|| not_found(path))?;
        if bytes.len() > MAX_IO_BYTES {
            return Err(too_large(MAX_IO_BYTES));
        }
        Ok(bytes.clone())
    }

    fn project_list(&mut self, dir: &str) -> Result<Vec<String>, Failure> {
        self.require(Capability::ProjectRead)?;
        // An empty `dir` is the root.
        let segments = if dir.is_empty() {
            Vec::new()
        } else {
            paths::relative_segments(dir).map_err(|refusal| refusal.failure(dir))?
        };
        if !segments
            .first()
            .is_some_and(|first| paths::is_config_dir(first))
            && !self.readable.iter().any(|path| path.starts_with(dir))
        {
            return Err(
                Failure::coded(FailureKind::AccessDenied, codes::PATH_NOT_GRANTED)
                    .with_param("path", dir),
            );
        }
        let prefix = if segments.is_empty() {
            String::new()
        } else {
            format!("{}/", segments.join("/"))
        };
        let names: BTreeSet<String> = self
            .project
            .keys()
            .filter_map(|path| path.strip_prefix(&prefix))
            .filter_map(|rest| rest.split('/').next())
            .filter(|name| {
                segments
                    .first()
                    .is_none_or(|first| !paths::is_local_config(&[first, name]))
            })
            .map(str::to_owned)
            .collect();
        Ok(names.into_iter().collect())
    }

    fn project_write(
        &mut self,
        path: &str,
        bytes: &[u8],
        expected: Option<&[u8]>,
    ) -> Result<(), Failure> {
        self.require(Capability::ProjectWrite)?;
        let segments = paths::relative_segments(path).map_err(|refusal| refusal.failure(path))?;
        if !self.trusted {
            return Err(
                Failure::coded(FailureKind::AccessDenied, codes::UNTRUSTED_WORKSPACE)
                    .with_param("path", path),
            );
        }
        let key = segments.join("/");
        // A write path must match a read glob too, as the host holds it.
        if paths::is_write_protected(&segments)
            || !self.writable.contains(&key)
            || !self.readable.contains(&key)
        {
            return Err(
                Failure::coded(FailureKind::AccessDenied, codes::PATH_NOT_WRITABLE)
                    .with_param("path", path),
            );
        }
        if bytes.len() > MAX_IO_BYTES || expected.is_some_and(|old| old.len() > MAX_IO_BYTES) {
            return Err(too_large(MAX_IO_BYTES));
        }
        if self.project.get(&key).map(Vec::as_slice) != expected {
            return Err(
                Failure::coded(FailureKind::Other, codes::WRITE_CONFLICT).with_param("path", path)
            );
        }
        self.project.insert(key, bytes.to_vec());
        Ok(())
    }

    fn out_write(&mut self, path: &str, bytes: &[u8]) -> Result<(), Failure> {
        self.require(Capability::OutWrite)?;
        let segments = paths::relative_segments(path).map_err(|refusal| refusal.failure(path))?;
        if bytes.len() > MAX_IO_BYTES {
            return Err(too_large(MAX_IO_BYTES));
        }
        self.out.insert(segments.join("/"), bytes.to_vec());
        Ok(())
    }

    fn report_findings(&mut self, items: &[Finding]) -> Result<(), Failure> {
        self.require(Capability::Findings)?;
        self.findings.extend_from_slice(items);
        Ok(())
    }

    fn sidecar_call(&mut self, method: &str, params: &str) -> Result<String, Failure> {
        self.require(Capability::Sidecar)?;
        let _ = (method, params);
        Ok(self.sidecar_answer.clone())
    }
}

#[cfg(test)]
#[path = "test_support_tests.rs"]
mod tests;
