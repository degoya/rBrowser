//! A host for the unit tests that answers `evaluate` from a script of answers, one per call (the
//! shared `MemoryHost` answers every call alike), with a clock that can move per call.

use std::collections::VecDeque;

use rb_plugin_api::{
    Capability, Failure, FailureKind, Finding, HttpRequest, HttpResponse, PluginHost, ViewId,
    ViewInfo, codes,
};

#[derive(Debug, Default)]
pub struct FakeHost {
    pub views: Vec<ViewInfo>,
    /// What the next `evaluate` calls answer, in order; `null` once used up.
    pub answers: VecDeque<Result<String, Failure>>,
    pub evaluated: Vec<(ViewId, String)>,
    pub now: u64,
    /// Seconds the clock moves at every reading.
    pub tick: u64,
    pub logs: Vec<(String, String)>,
}

impl FakeHost {
    pub fn answer(&mut self, json: &str) {
        self.answers.push_back(Ok(json.to_owned()));
    }

    pub fn fail(&mut self, code: &str) {
        self.answers
            .push_back(Err(Failure::coded(FailureKind::Other, code)));
    }
}

impl PluginHost for FakeHost {
    fn log(&mut self, level: &str, message: &str) {
        self.logs.push((level.to_owned(), message.to_owned()));
    }

    fn now_unix_seconds(&mut self) -> u64 {
        self.now += self.tick;
        self.now
    }

    fn setting(&mut self, _key: &str) -> Option<String> {
        None
    }

    fn views(&mut self) -> Result<Vec<ViewInfo>, Failure> {
        Ok(self.views.clone())
    }

    fn active_view(&mut self) -> Result<Option<ViewId>, Failure> {
        Ok(None)
    }

    fn evaluate(&mut self, view: ViewId, script: &str) -> Result<String, Failure> {
        self.evaluated.push((view, script.to_owned()));
        self.answers
            .pop_front()
            .unwrap_or_else(|| Ok("null".to_owned()))
    }

    fn cdp_send(&mut self, _view: ViewId, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Cdp.denied())
    }

    fn fetch(&mut self, _request: &HttpRequest) -> Result<HttpResponse, Failure> {
        Err(Capability::Http.denied())
    }

    fn project_read(&mut self, _path: &str) -> Result<Vec<u8>, Failure> {
        Err(Failure::coded(FailureKind::NotFound, codes::NOT_FOUND))
    }

    fn project_list(&mut self, _dir: &str) -> Result<Vec<String>, Failure> {
        Err(Capability::ProjectRead.denied())
    }

    fn out_write(&mut self, _path: &str, _bytes: &[u8]) -> Result<(), Failure> {
        Ok(())
    }

    fn report_findings(&mut self, _items: &[Finding]) -> Result<(), Failure> {
        Ok(())
    }

    fn sidecar_call(&mut self, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Sidecar.denied())
    }
}
