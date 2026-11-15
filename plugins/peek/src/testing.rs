//! A host for the native tests: CDP answers by method, evaluate answers by view, views and
//! settings in memory, and every call recorded. `MemoryHost` of `rb-plugin-api` answers one text
//! for every CDP method; peek needs one per method.

use std::collections::{BTreeMap, VecDeque};

use rb_plugin_api::{
    Capability, Failure, FailureKind, Finding, HttpRequest, HttpResponse, PluginHost, ViewId,
    ViewInfo, codes,
};
use serde_json::Value;

#[derive(Debug, Default)]
pub struct FakeHost {
    /// CDP answers by method; an unknown method answers `plugins.error.not_available`.
    pub cdp: BTreeMap<String, Result<Value, Failure>>,
    /// One-time CDP answers by method, used up in order before [`FakeHost::cdp`] answers.
    pub queued: BTreeMap<String, VecDeque<Result<Value, Failure>>>,
    /// Every CDP command, as `(view, method, params)`.
    pub cdp_sent: Vec<(ViewId, String, Value)>,
    /// What `evaluate` answers, by view (JSON text); an unknown view answers not found.
    pub evaluate: BTreeMap<ViewId, String>,
    pub evaluated: Vec<(ViewId, String)>,
    pub views: Vec<ViewInfo>,
    /// Settings as JSON text by property name.
    pub settings: BTreeMap<String, String>,
}

impl FakeHost {
    pub fn answer(&mut self, method: &str, value: Value) -> &mut Self {
        self.cdp.insert(method.to_owned(), Ok(value));
        self
    }

    pub fn fail(&mut self, method: &str) -> &mut Self {
        self.cdp.insert(
            method.to_owned(),
            Err(Failure::coded(
                FailureKind::Other,
                "plugins.error.host_failed",
            )),
        );
        self
    }

    /// The next call of `method` fails once; later calls answer as before.
    pub fn fail_once(&mut self, method: &str) -> &mut Self {
        self.queued
            .entry(method.to_owned())
            .or_default()
            .push_back(Err(Failure::coded(
                FailureKind::Other,
                "plugins.error.host_failed",
            )));
        self
    }

    pub fn view(&mut self, id: ViewId, engine: &str) -> &mut Self {
        self.views.push(ViewInfo {
            id,
            label: format!("View {id}"),
            breakpoint: "bp".to_owned(),
            engine: engine.to_owned(),
            width: 800,
            height: 600,
            dpr: 1.0,
            url: "http://localhost/".to_owned(),
            title: String::new(),
        });
        self
    }

    pub fn methods(&self) -> Vec<&str> {
        self.cdp_sent
            .iter()
            .map(|(_, method, _)| method.as_str())
            .collect()
    }
}

impl PluginHost for FakeHost {
    fn log(&mut self, _level: &str, _message: &str) {}

    fn now_unix_seconds(&mut self) -> u64 {
        0
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
        self.evaluate
            .get(&view)
            .cloned()
            .ok_or_else(|| Failure::coded(FailureKind::NotFound, codes::NOT_FOUND))
    }

    fn cdp_send(&mut self, view: ViewId, method: &str, params: &str) -> Result<String, Failure> {
        let params = serde_json::from_str(params).expect("CDP parameters are JSON");
        self.cdp_sent.push((view, method.to_owned(), params));
        if let Some(answer) = self.queued.get_mut(method).and_then(VecDeque::pop_front) {
            return answer.map(|value| value.to_string());
        }
        match self.cdp.get(method) {
            Some(Ok(value)) => Ok(value.to_string()),
            Some(Err(failure)) => Err(failure.clone()),
            None => Err(Failure::coded(FailureKind::Other, codes::NOT_AVAILABLE)),
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

    fn report_findings(&mut self, _items: &[Finding]) -> Result<(), Failure> {
        Err(Capability::Findings.denied())
    }

    fn sidecar_call(&mut self, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Sidecar.denied())
    }
}
