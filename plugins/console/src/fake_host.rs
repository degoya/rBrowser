//! A host for the unit tests: views with an answer of their own for `evaluate` (the shared
//! `MemoryHost` answers every view alike), settings, and one file store behind `out-write` and
//! `project-read`, as the real host's out folder lies below `.rbrowser/`.

use std::collections::BTreeMap;

use rb_plugin_api::{
    Capability, Failure, Finding, HttpRequest, HttpResponse, PluginHost, ViewId, ViewInfo,
};

#[derive(Debug, Default)]
pub struct FakeHost {
    pub views: Vec<ViewInfo>,
    pub active: Option<ViewId>,
    /// What `evaluate` answers per view; a view without one answers `null`.
    pub answers: BTreeMap<ViewId, Result<String, Failure>>,
    pub settings: BTreeMap<String, String>,
    /// Project files by project-relative path.
    pub files: BTreeMap<String, Vec<u8>>,
    pub now: u64,
    pub logs: Vec<(String, String)>,
    pub evaluated: Vec<(ViewId, String)>,
    /// `out.write` fails like a host without a project.
    pub out_fails: bool,
}

/// A view of `width` pixels.
pub fn view(id: ViewId, label: &str, width: u32) -> ViewInfo {
    ViewInfo {
        id,
        label: label.to_owned(),
        breakpoint: label.to_lowercase(),
        engine: "chromium".to_owned(),
        width,
        height: 800,
        dpr: 1.0,
        url: "http://127.0.0.1:8080/console.html".to_owned(),
        title: "Console fixture".to_owned(),
    }
}

impl FakeHost {
    pub fn with_views(views: Vec<ViewInfo>) -> Self {
        Self {
            views,
            now: 1_731_300_000,
            ..Self::default()
        }
    }

    pub fn set(&mut self, key: &str, json: &str) {
        self.settings.insert(key.to_owned(), json.to_owned());
    }
}

impl PluginHost for FakeHost {
    fn log(&mut self, level: &str, message: &str) {
        self.logs.push((level.to_owned(), message.to_owned()));
    }

    fn now_unix_seconds(&mut self) -> u64 {
        self.now
    }

    fn setting(&mut self, key: &str) -> Option<String> {
        self.settings.get(key).cloned()
    }

    fn views(&mut self) -> Result<Vec<ViewInfo>, Failure> {
        Ok(self.views.clone())
    }

    fn active_view(&mut self) -> Result<Option<ViewId>, Failure> {
        Ok(self.active)
    }

    fn evaluate(&mut self, view: ViewId, script: &str) -> Result<String, Failure> {
        self.evaluated.push((view, script.to_owned()));
        self.answers
            .get(&view)
            .cloned()
            .unwrap_or_else(|| Ok("null".to_owned()))
    }

    fn cdp_send(&mut self, _view: ViewId, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Cdp.denied())
    }

    fn fetch(&mut self, _request: &HttpRequest) -> Result<HttpResponse, Failure> {
        Err(Capability::Http.denied())
    }

    fn project_read(&mut self, path: &str) -> Result<Vec<u8>, Failure> {
        self.files.get(path).cloned().ok_or_else(|| {
            Failure::coded(
                rb_plugin_api::FailureKind::NotFound,
                rb_plugin_api::codes::NOT_FOUND,
            )
        })
    }

    fn project_list(&mut self, _dir: &str) -> Result<Vec<String>, Failure> {
        Err(Capability::ProjectRead.denied())
    }

    fn out_write(&mut self, path: &str, bytes: &[u8]) -> Result<(), Failure> {
        if self.out_fails {
            return Err(Failure::coded(
                rb_plugin_api::FailureKind::NotSupported,
                rb_plugin_api::codes::NOT_AVAILABLE,
            ));
        }
        self.files
            .insert(format!(".rbrowser/out/console/{path}"), bytes.to_vec());
        Ok(())
    }

    fn report_findings(&mut self, _items: &[Finding]) -> Result<(), Failure> {
        Err(Capability::Findings.denied())
    }

    fn sidecar_call(&mut self, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Sidecar.denied())
    }
}
