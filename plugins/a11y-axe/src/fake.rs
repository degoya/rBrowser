//! A host for the native tests: views with their own `evaluate` answers (the stored runner
//! answers of `tests/fixtures/`), settings, and a record of what the plugin asked for.
//! `rb_plugin_api::test_support::MemoryHost` answers every view the same; a run over several
//! breakpoints needs one answer per view.

use std::collections::BTreeMap;

use rb_plugin_api::{
    Capability, Failure, FailureKind, Finding, HttpRequest, HttpResponse, PluginHost, ViewId,
    ViewInfo,
};

/// A stored runner answer from `tests/fixtures/<name>.json`.
pub fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).expect("the fixture exists")
}

/// The fake host.
#[derive(Default)]
pub struct FakeHost {
    pub views: Vec<ViewInfo>,
    /// Per view: the text `evaluate` answers, or its failure.
    pub answers: BTreeMap<ViewId, Result<String, Failure>>,
    /// Settings by key, JSON text.
    pub settings: BTreeMap<String, String>,
    /// Whether `findings.report` is wired; `plugins.error.not_available` when not.
    pub findings_wired: bool,
    pub evaluated: Vec<(ViewId, String)>,
    pub reported: Vec<Finding>,
    pub progress: Vec<(u32, u32)>,
    pub logs: Vec<(String, String)>,
}

impl FakeHost {
    /// A host with `views` as (id, breakpoint, fixture).
    pub fn with_views(views: &[(ViewId, &str, &str)]) -> Self {
        let mut host = Self {
            findings_wired: true,
            ..Self::default()
        };
        for (id, breakpoint, answer) in views {
            host.views.push(ViewInfo {
                id: *id,
                label: format!("View {id}"),
                breakpoint: (*breakpoint).to_owned(),
                engine: "chromium".to_owned(),
                width: 375,
                height: 800,
                dpr: 1.0,
                url: "http://localhost:5173/a11y.html".to_owned(),
                title: "a11y fixture".to_owned(),
            });
            host.answers.insert(*id, Ok(fixture(answer)));
        }
        host
    }

    /// Sets the plugin's setting `key` to the JSON `value`.
    pub fn set(mut self, key: &str, value: &str) -> Self {
        self.settings.insert(key.to_owned(), value.to_owned());
        self
    }
}

impl PluginHost for FakeHost {
    fn log(&mut self, level: &str, message: &str) {
        self.logs.push((level.to_owned(), message.to_owned()));
    }

    fn now_unix_seconds(&mut self) -> u64 {
        0
    }

    fn report_progress(&mut self, done: u32, total: u32) {
        self.progress.push((done, total));
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

    fn evaluate(&mut self, view: ViewId, script: &str) -> Result<String, Failure> {
        self.evaluated.push((view, script.to_owned()));
        self.answers
            .get(&view)
            .cloned()
            .unwrap_or_else(|| Err(Failure::new(FailureKind::NotFound)))
    }

    fn cdp_send(&mut self, _view: ViewId, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Cdp.denied())
    }

    fn fetch(&mut self, _request: &HttpRequest) -> Result<HttpResponse, Failure> {
        Err(Capability::Http.denied())
    }

    fn project_read(&mut self, _path: &str) -> Result<Vec<u8>, Failure> {
        Err(Capability::ProjectRead.denied())
    }

    fn project_list(&mut self, _dir: &str) -> Result<Vec<String>, Failure> {
        Err(Capability::ProjectRead.denied())
    }

    fn out_write(&mut self, _path: &str, _bytes: &[u8]) -> Result<(), Failure> {
        Err(Capability::OutWrite.denied())
    }

    fn report_findings(&mut self, items: &[Finding]) -> Result<(), Failure> {
        if !self.findings_wired {
            return Err(Failure::coded(
                FailureKind::NotSupported,
                rb_plugin_api::codes::NOT_AVAILABLE,
            ));
        }
        self.reported.extend_from_slice(items);
        Ok(())
    }

    fn sidecar_call(&mut self, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Sidecar.denied())
    }
}
