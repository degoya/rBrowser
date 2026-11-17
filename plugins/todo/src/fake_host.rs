//! A host for the unit tests that plays the real host's rules for this plugin's grants
//! (`project-read` of `**/*.todo` and `**/.gitignore`, `project-write` of `**/*.todo`): folders
//! listed with a trailing `/` (the root too), `.todo` and `.gitignore` files read, `.todo` files
//! written only in a trusted workspace and only over the bytes read
//! (`plugins.error.write_conflict`), never into `.git/` or `.rbrowser/`.

use std::collections::{BTreeMap, BTreeSet};

use rb_plugin_api::{
    Capability, Failure, FailureKind, Finding, HttpRequest, HttpResponse, PluginHost, ViewId,
    ViewInfo, codes, paths,
};

#[derive(Debug, Default)]
pub struct FakeHost {
    /// Project files by project-relative path.
    pub files: BTreeMap<String, Vec<u8>>,
    pub trusted: bool,
    /// No project is open: every file call answers `plugins.error.not_available`.
    pub no_project: bool,
    pub settings: BTreeMap<String, String>,
    pub now: u64,
    /// Every path written, in order.
    pub written: Vec<String>,
    /// Every folder listed, in order.
    pub listed: Vec<String>,
}

impl FakeHost {
    /// A trusted project with `files`.
    pub fn with(files: &[(&str, &str)]) -> Self {
        Self {
            files: files
                .iter()
                .map(|(path, text)| ((*path).to_owned(), text.as_bytes().to_vec()))
                .collect(),
            trusted: true,
            ..Self::default()
        }
    }

    pub fn text(&self, path: &str) -> String {
        String::from_utf8(self.files[path].clone()).expect("UTF-8")
    }

    fn project(&self) -> Result<(), Failure> {
        if self.no_project {
            Err(
                Failure::coded(FailureKind::NotSupported, codes::NOT_AVAILABLE)
                    .with_param("service", "project"),
            )
        } else {
            Ok(())
        }
    }
}

/// Whether the read grant covers `path`: a todo file or a `.gitignore`.
fn readable(path: &str) -> bool {
    path.ends_with(".todo") || path.rsplit('/').next() == Some(".gitignore")
}

fn not_found(path: &str) -> Failure {
    Failure::coded(FailureKind::NotFound, codes::NOT_FOUND).with_param("path", path)
}

impl PluginHost for FakeHost {
    fn log(&mut self, _level: &str, _message: &str) {}

    fn now_unix_seconds(&mut self) -> u64 {
        self.now
    }

    fn setting(&mut self, key: &str) -> Option<String> {
        self.settings.get(key).cloned()
    }

    fn views(&mut self) -> Result<Vec<ViewInfo>, Failure> {
        Err(Capability::Views.denied())
    }

    fn active_view(&mut self) -> Result<Option<ViewId>, Failure> {
        Err(Capability::Views.denied())
    }

    fn evaluate(&mut self, _view: ViewId, _script: &str) -> Result<String, Failure> {
        Err(Capability::Evaluate.denied())
    }

    fn cdp_send(&mut self, _view: ViewId, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Cdp.denied())
    }

    fn fetch(&mut self, _request: &HttpRequest) -> Result<HttpResponse, Failure> {
        Err(Capability::Http.denied())
    }

    fn project_read(&mut self, path: &str) -> Result<Vec<u8>, Failure> {
        self.project()?;
        paths::relative_segments(path).map_err(|refusal| refusal.failure(path))?;
        if !readable(path) {
            return Err(
                Failure::coded(FailureKind::AccessDenied, codes::PATH_NOT_GRANTED)
                    .with_param("path", path),
            );
        }
        self.files.get(path).cloned().ok_or_else(|| not_found(path))
    }

    fn project_list(&mut self, dir: &str) -> Result<Vec<String>, Failure> {
        self.project()?;
        self.listed.push(dir.to_owned());
        let prefix = if dir.is_empty() {
            String::new()
        } else {
            format!("{dir}/")
        };
        let names: BTreeSet<String> = self
            .files
            .keys()
            .filter_map(|path| path.strip_prefix(&prefix))
            .filter_map(|rest| match rest.split_once('/') {
                Some((folder, _)) => Some(format!("{folder}/")),
                None => readable(rest).then(|| rest.to_owned()),
            })
            .collect();
        if names.is_empty() && !dir.is_empty() {
            return Err(not_found(dir));
        }
        Ok(names.into_iter().collect())
    }

    fn project_write(
        &mut self,
        path: &str,
        bytes: &[u8],
        expected: Option<&[u8]>,
    ) -> Result<(), Failure> {
        self.project()?;
        let segments = paths::relative_segments(path).map_err(|refusal| refusal.failure(path))?;
        if !self.trusted {
            return Err(
                Failure::coded(FailureKind::AccessDenied, codes::UNTRUSTED_WORKSPACE)
                    .with_param("path", path),
            );
        }
        if paths::is_write_protected(&segments) || !path.ends_with(".todo") {
            return Err(
                Failure::coded(FailureKind::AccessDenied, codes::PATH_NOT_WRITABLE)
                    .with_param("path", path),
            );
        }
        if self.files.get(path).map(Vec::as_slice) != expected {
            return Err(
                Failure::coded(FailureKind::Other, codes::WRITE_CONFLICT).with_param("path", path)
            );
        }
        self.files.insert(path.to_owned(), bytes.to_vec());
        self.written.push(path.to_owned());
        Ok(())
    }

    fn out_write(&mut self, _path: &str, _bytes: &[u8]) -> Result<(), Failure> {
        Err(Capability::OutWrite.denied())
    }

    fn report_findings(&mut self, _items: &[Finding]) -> Result<(), Failure> {
        Err(Capability::Findings.denied())
    }

    fn sidecar_call(&mut self, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Sidecar.denied())
    }
}
