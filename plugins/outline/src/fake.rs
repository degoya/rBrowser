//! A host for the native tests: views, an evaluate answer per view, CDP answers by method, HTTP
//! answers by method and URL, a clock that may run, and every call recorded. `MemoryHost` of
//! `rb-plugin-api` answers one text for every view and every method; the outline needs one each.

use std::collections::BTreeMap;

use rb_plugin_api::{
    Capability, Failure, FailureKind, Finding, HttpRequest, HttpResponse, PluginHost, ViewId,
    ViewInfo, codes,
};
use serde_json::Value;

#[derive(Debug, Default)]
pub struct FakeHost {
    pub views: Vec<ViewInfo>,
    /// What the collector answers, by view (JSON text); an unknown view answers not found.
    pub pages: BTreeMap<ViewId, String>,
    /// Every script evaluated, as `(view, script)`.
    pub evaluated: Vec<(ViewId, String)>,
    /// CDP answers by method; an unknown method answers `{}`.
    pub cdp: BTreeMap<String, Result<Value, Failure>>,
    pub cdp_sent: Vec<(ViewId, String, Value)>,
    /// HTTP answers by `"<METHOD> <url>"`; an unknown one answers 404.
    pub responses: BTreeMap<String, Result<HttpResponse, Failure>>,
    pub fetched: Vec<(String, String)>,
    pub settings: BTreeMap<String, String>,
    /// The clock, and how far it moves at each reading.
    pub now: u64,
    pub tick: u64,
    pub findings: Vec<Finding>,
}

impl FakeHost {
    /// A Chromium view `id` labelled `label` showing `url`.
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

    pub fn page(&mut self, view: ViewId, outline: &Value) -> &mut Self {
        self.pages.insert(view, outline.to_string());
        self
    }

    pub fn respond(&mut self, method: &str, url: &str, status: u16) -> &mut Self {
        self.responses.insert(
            format!("{method} {url}"),
            Ok(HttpResponse {
                status,
                headers: Vec::new(),
                body: Vec::new(),
            }),
        );
        self
    }

    pub fn redirect(&mut self, method: &str, url: &str, to: &str) -> &mut Self {
        self.responses.insert(
            format!("{method} {url}"),
            Ok(HttpResponse {
                status: 301,
                headers: vec![("location".to_owned(), to.to_owned())],
                body: Vec::new(),
            }),
        );
        self
    }

    pub fn refuse(&mut self, method: &str, url: &str, code: &str) -> &mut Self {
        self.responses.insert(
            format!("{method} {url}"),
            Err(Failure::coded(FailureKind::Other, code)),
        );
        self
    }

    /// The waits the link check spent in views, in milliseconds.
    pub fn waits(&self) -> Vec<String> {
        self.evaluated
            .iter()
            .filter(|(_, script)| script.starts_with("new Promise((done) => setTimeout"))
            .map(|(_, script)| script.clone())
            .collect()
    }

    pub fn methods(&self) -> Vec<&str> {
        self.cdp_sent.iter().map(|(_, m, _)| m.as_str()).collect()
    }
}

impl PluginHost for FakeHost {
    fn log(&mut self, _level: &str, _message: &str) {}

    fn now_unix_seconds(&mut self) -> u64 {
        let now = self.now;
        self.now += self.tick;
        now
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
        if script.starts_with("new Promise((done) => setTimeout") && self.pages.contains_key(&view)
        {
            return Ok("null".to_owned());
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

    fn fetch(&mut self, request: &HttpRequest) -> Result<HttpResponse, Failure> {
        self.fetched
            .push((request.method.clone(), request.url.clone()));
        self.responses
            .get(&format!("{} {}", request.method, request.url))
            .cloned()
            .unwrap_or(Ok(HttpResponse {
                status: 404,
                ..HttpResponse::default()
            }))
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
        self.findings.extend_from_slice(items);
        Ok(())
    }

    fn sidecar_call(&mut self, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Sidecar.denied())
    }
}
