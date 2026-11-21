//! A host for the native tests (after the outline plugin's): views with their density, an
//! evaluate answer per view (a view without one answers not found, a Firefox view not available),
//! CDP answers by method, and every call recorded. `MemoryHost` of `rb-plugin-api` answers one
//! text for every view and every method; this plugin needs one each.

use std::collections::BTreeMap;

use rb_plugin_api::{
    Capability, Failure, FailureKind, Finding, HttpRequest, HttpResponse, PluginHost, ViewId,
    ViewInfo, codes,
};
use serde_json::Value;

#[derive(Debug, Default)]
pub struct FakeHost {
    pub views: Vec<ViewInfo>,
    pub active: Option<ViewId>,
    /// What the collector answers, by view (JSON text).
    pub pages: BTreeMap<ViewId, String>,
    /// Every script evaluated, as `(view, script)`.
    pub evaluated: Vec<(ViewId, String)>,
    /// CDP answers by method; an unknown method answers `{}`.
    pub cdp: BTreeMap<String, Result<Value, Failure>>,
    pub cdp_sent: Vec<(ViewId, String, Value)>,
    pub settings: BTreeMap<String, String>,
    pub findings: Vec<Finding>,
    pub progress: Vec<(u32, u32)>,
    /// `report_findings` fails.
    pub refuse_findings: bool,
}

impl FakeHost {
    /// A Chromium view `id` labelled `label` showing `url` at density 1.
    pub fn view(&mut self, id: ViewId, label: &str, url: &str) -> &mut Self {
        self.views.push(ViewInfo {
            id,
            label: label.to_owned(),
            breakpoint: label.to_ascii_lowercase(),
            engine: "chromium".to_owned(),
            width: 1280,
            height: 800,
            dpr: 1.0,
            url: url.to_owned(),
            title: String::new(),
        });
        self
    }

    /// The last view added gets `engine`, `width` and `dpr`.
    pub fn shaped(&mut self, engine: &str, width: u32, dpr: f32) -> &mut Self {
        if let Some(view) = self.views.last_mut() {
            engine.clone_into(&mut view.engine);
            view.width = width;
            view.dpr = dpr;
        }
        self
    }

    pub fn page(&mut self, view: ViewId, answer: &Value) -> &mut Self {
        self.pages.insert(view, answer.to_string());
        self
    }

    pub fn methods(&self) -> Vec<&str> {
        self.cdp_sent.iter().map(|(_, m, _)| m.as_str()).collect()
    }
}

impl PluginHost for FakeHost {
    fn log(&mut self, _level: &str, _message: &str) {}

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
        Ok(self
            .active
            .or_else(|| self.views.first().map(|view| view.id)))
    }

    fn evaluate(&mut self, view: ViewId, script: &str) -> Result<String, Failure> {
        self.evaluated.push((view, script.to_owned()));
        if let Some(found) = self.views.iter().find(|one| one.id == view)
            && found.engine != "chromium"
        {
            return Err(Failure::engine_unserved("evaluate", view, &found.engine));
        }
        self.pages
            .get(&view)
            .cloned()
            .ok_or_else(|| Failure::coded(FailureKind::NotFound, codes::NOT_FOUND))
    }

    fn cdp_send(&mut self, view: ViewId, method: &str, params: &str) -> Result<String, Failure> {
        let params: Value = serde_json::from_str(params).unwrap_or(Value::Null);
        self.cdp_sent.push((view, method.to_owned(), params));
        match self.cdp.get(method) {
            Some(Ok(value)) => Ok(value.to_string()),
            Some(Err(failure)) => Err(failure.clone()),
            None => Ok("{}".to_owned()),
        }
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
        if self.refuse_findings {
            return Err(Capability::Findings.denied());
        }
        self.findings.extend_from_slice(items);
        Ok(())
    }

    fn sidecar_call(&mut self, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Sidecar.denied())
    }
}
