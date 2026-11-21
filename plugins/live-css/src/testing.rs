//! A host for the unit tests: views, settings, the project's files and what `out.write` wrote,
//! and a workspace that may be untrusted or missing - the trust probe then answers
//! `plugins.error.untrusted_workspace` or `plugins.error.not_available`, as the real host does.

use std::collections::BTreeMap;

use rb_plugin_api::{
    Capability, Failure, FailureKind, Finding, HttpRequest, HttpResponse, PluginHost, ViewId,
    ViewInfo, codes,
};
use serde_json::Value;

use crate::store::{DRAFT_PATH, SHEET_PATH, TRUST_PROBE};

#[derive(Debug, Default)]
pub struct FakeHost {
    pub views: Vec<ViewInfo>,
    pub settings: BTreeMap<String, String>,
    /// Project files by project-relative path; `out.write` lands below `.rbrowser/out/live-css/`.
    pub files: BTreeMap<String, Vec<u8>>,
    pub trusted: bool,
    /// No project is open: every file call answers `plugins.error.not_available`.
    pub no_project: bool,
    /// How many times `out.write` was called.
    pub writes: usize,
    pub logs: Vec<(String, String)>,
}

/// A Chromium view of breakpoint `breakpoint` at `width` CSS px.
pub fn view(id: ViewId, breakpoint: &str, width: u32) -> ViewInfo {
    ViewInfo {
        id,
        label: breakpoint.to_uppercase(),
        breakpoint: breakpoint.to_owned(),
        engine: "chromium".to_owned(),
        width,
        height: 800,
        dpr: 1.0,
        url: "http://example.test/".to_owned(),
        title: "Fixture".to_owned(),
    }
}

/// A trusted workspace with the views `mobile` (375), `md` (768) and `lg` (1280).
pub fn host() -> FakeHost {
    FakeHost {
        views: vec![
            view(1, "mobile", 375),
            view(2, "md", 768),
            view(3, "lg", 1280),
        ],
        trusted: true,
        ..FakeHost::default()
    }
}

pub fn json(text: &str) -> Value {
    serde_json::from_str(text).expect("JSON answer")
}

impl FakeHost {
    pub fn set(&mut self, key: &str, json: &str) {
        self.settings.insert(key.to_owned(), json.to_owned());
    }

    pub fn file(&mut self, path: &str, text: &str) {
        self.files.insert(path.to_owned(), text.as_bytes().to_vec());
    }

    /// The stored draft, as JSON.
    pub fn draft(&self) -> Value {
        let bytes = self.files.get(DRAFT_PATH).expect("draft.json written");
        serde_json::from_slice(bytes).expect("draft.json is JSON")
    }

    /// The stored stylesheet.
    pub fn sheet_file(&self) -> String {
        let bytes = self.files.get(SHEET_PATH).expect("draft.css written");
        String::from_utf8(bytes.clone()).expect("draft.css is UTF-8")
    }

    fn gate(&self) -> Result<(), Failure> {
        if self.no_project {
            return Err(Failure::coded(
                FailureKind::NotSupported,
                codes::NOT_AVAILABLE,
            ));
        }
        Ok(())
    }
}

impl PluginHost for FakeHost {
    fn log(&mut self, level: &str, message: &str) {
        self.logs.push((level.to_owned(), message.to_owned()));
    }

    fn now_unix_seconds(&mut self) -> u64 {
        1_763_600_000
    }

    fn setting(&mut self, key: &str) -> Option<String> {
        self.settings.get(key).cloned()
    }

    fn views(&mut self) -> Result<Vec<ViewInfo>, Failure> {
        Ok(self.views.clone())
    }

    fn active_view(&mut self) -> Result<Option<ViewId>, Failure> {
        Ok(self.views.first().map(|view| view.id))
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
        self.gate()?;
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| Failure::coded(FailureKind::NotFound, codes::NOT_FOUND))
    }

    fn project_list(&mut self, dir: &str) -> Result<Vec<String>, Failure> {
        self.gate()?;
        if !self.trusted && dir == TRUST_PROBE {
            return Err(Failure::coded(
                FailureKind::AccessDenied,
                codes::UNTRUSTED_WORKSPACE,
            ));
        }
        let prefix = format!("{dir}/");
        let names: Vec<String> = self
            .files
            .keys()
            .filter_map(|path| path.strip_prefix(&prefix))
            .map(str::to_owned)
            .collect();
        if names.is_empty() {
            return Err(Failure::coded(FailureKind::NotFound, codes::NOT_FOUND));
        }
        Ok(names)
    }

    fn out_write(&mut self, path: &str, bytes: &[u8]) -> Result<(), Failure> {
        self.gate()?;
        self.writes += 1;
        self.files
            .insert(format!(".rbrowser/out/live-css/{path}"), bytes.to_vec());
        Ok(())
    }

    fn report_findings(&mut self, _items: &[Finding]) -> Result<(), Failure> {
        Err(Capability::Findings.denied())
    }

    fn sidecar_call(&mut self, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Sidecar.denied())
    }
}
